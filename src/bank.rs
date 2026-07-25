use crate::error::AudioError;
use crate::tier::EffectKind;

pub const BANK_MAGIC: [u8; 4] = *b"EAFX";
pub const BANK_VERSION: u8 = 2;
pub const HEADER_SIZE: usize = 10;
pub const ENTRY_SIZE: usize = 16;
/// Maximum bank blob size for [`BankBuilder`] (tune per product flash budget).
pub const BANK_BUILD_CAP: usize = 65_536;

const MAX_EFFECTS: usize = 64;

/// Parsed view over a contiguous effect bank blob in flash or ROM.
#[derive(Debug, Clone, Copy)]
pub struct SoundBank<'a> {
    data: &'a [u8],
    pub sample_rate_hz: u32,
    effect_count: u16,
}

/// Directory entry describing one effect payload.
#[derive(Debug, Clone, Copy)]
pub struct EffectEntry {
    pub id: u16,
    pub kind: EffectKind,
    pub flags: u8,
    pub default_gain_q8: u8,
    pub param0: u16,
    pub param1: u16,
    pub offset: u32,
    pub len: u32,
}

impl EffectEntry {
    pub fn payload<'a>(&self, bank: &'a SoundBank<'_>) -> Result<&'a [u8], AudioError> {
        bank.payload(self)
    }
}

impl<'a> SoundBank<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, AudioError> {
        if data.len() < HEADER_SIZE {
            return Err(AudioError::TruncatedBank);
        }
        if data[0..4] != BANK_MAGIC {
            return Err(AudioError::InvalidBankMagic);
        }
        if data[4] != BANK_VERSION {
            return Err(AudioError::UnsupportedBankVersion);
        }
        let effect_count = u16::from_le_bytes([data[5], data[6]]);
        let sample_rate_hz = u16::from_le_bytes([data[7], data[8]]) as u32;
        let sample_rate_hz = if sample_rate_hz == 0 {
            crate::config::DEFAULT_SAMPLE_RATE_HZ
        } else {
            sample_rate_hz
        };
        let needed = HEADER_SIZE + effect_count as usize * ENTRY_SIZE;
        if data.len() < needed {
            return Err(AudioError::TruncatedBank);
        }
        Ok(Self {
            data,
            sample_rate_hz,
            effect_count,
        })
    }

    pub const fn effect_count(&self) -> u16 {
        self.effect_count
    }

    pub fn entry(&self, index: usize) -> Result<EffectEntry, AudioError> {
        if index >= self.effect_count as usize {
            return Err(AudioError::EffectNotFound);
        }
        let off = HEADER_SIZE + index * ENTRY_SIZE;
        let slice = &self.data[off..off + ENTRY_SIZE];
        let id = u16::from_le_bytes([slice[0], slice[1]]);
        let kind = EffectKind::from_u8(slice[2]).ok_or(AudioError::InvalidEffectKind)?;
        let flags = slice[3];
        let default_gain_q8 = slice[4];
        let param0 = u16::from_le_bytes([slice[6], slice[7]]);
        let param1 = u16::from_le_bytes([slice[8], slice[9]]);
        let offset = u32::from_le_bytes([slice[10], slice[11], slice[12], slice[13]]);
        let len = u16::from_le_bytes([slice[14], slice[15]]) as u32;
        Ok(EffectEntry {
            id,
            kind,
            flags,
            default_gain_q8,
            param0,
            param1,
            offset,
            len,
        })
    }

    pub fn find_by_id(&self, id: u16) -> Result<EffectEntry, AudioError> {
        for i in 0..self.effect_count as usize {
            let e = self.entry(i)?;
            if e.id == id {
                return Ok(e);
            }
        }
        Err(AudioError::EffectNotFound)
    }

    pub fn payload(&self, entry: &EffectEntry) -> Result<&'a [u8], AudioError> {
        let start = entry.offset as usize;
        let end = start + entry.len as usize;
        self.data.get(start..end).ok_or(AudioError::TruncatedBank)
    }
}

/// Build an effect bank blob (host tooling / tests).
pub struct BankBuilder {
    sample_rate_hz: u32,
    entries: heapless::Vec<EffectEntry, MAX_EFFECTS>,
    payload: heapless::Vec<u8, { BANK_BUILD_CAP }>,
}

impl BankBuilder {
    pub fn new(sample_rate_hz: u32) -> Self {
        Self {
            sample_rate_hz,
            entries: heapless::Vec::new(),
            payload: heapless::Vec::new(),
        }
    }

    pub fn add_effect(
        &mut self,
        id: u16,
        kind: EffectKind,
        flags: u8,
        default_gain_q8: u8,
        param0: u16,
        param1: u16,
        bytes: &[u8],
    ) -> Result<(), AudioError> {
        if self.entries.len() >= MAX_EFFECTS {
            return Err(AudioError::BankFull);
        }
        let offset =
            (HEADER_SIZE + (self.entries.len() + 1) * ENTRY_SIZE + self.payload.len()) as u32;
        self.payload
            .extend_from_slice(bytes)
            .map_err(|_| AudioError::BankFull)?;
        self.entries
            .push(EffectEntry {
                id,
                kind,
                flags,
                default_gain_q8,
                param0,
                param1,
                offset,
                len: bytes.len() as u32,
            })
            .map_err(|_| AudioError::BankFull)?;
        Ok(())
    }

    pub fn finish(
        &self,
        out: &mut heapless::Vec<u8, { BANK_BUILD_CAP }>,
    ) -> Result<(), AudioError> {
        out.clear();
        out.extend_from_slice(&BANK_MAGIC)
            .map_err(|_| AudioError::BankFull)?;
        out.push(BANK_VERSION).map_err(|_| AudioError::BankFull)?;
        let count = self.entries.len() as u16;
        out.push((count & 0xFF) as u8)
            .map_err(|_| AudioError::BankFull)?;
        out.push((count >> 8) as u8)
            .map_err(|_| AudioError::BankFull)?;
        let rate = self.sample_rate_hz.min(u16::MAX as u32) as u16;
        out.push((rate & 0xFF) as u8)
            .map_err(|_| AudioError::BankFull)?;
        out.push((rate >> 8) as u8)
            .map_err(|_| AudioError::BankFull)?;
        out.push(0).map_err(|_| AudioError::BankFull)?; // reserved
        for e in &self.entries {
            out.push((e.id & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push((e.id >> 8) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push(e.kind as u8).map_err(|_| AudioError::BankFull)?;
            out.push(e.flags).map_err(|_| AudioError::BankFull)?;
            out.push(e.default_gain_q8)
                .map_err(|_| AudioError::BankFull)?;
            out.push(0).map_err(|_| AudioError::BankFull)?;
            out.push((e.param0 & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push((e.param0 >> 8) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push((e.param1 & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push((e.param1 >> 8) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push((e.offset & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push(((e.offset >> 8) & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push(((e.offset >> 16) & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push(((e.offset >> 24) & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push((e.len & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
            out.push(((e.len >> 8) & 0xFF) as u8)
                .map_err(|_| AudioError::BankFull)?;
        }
        out.extend_from_slice(&self.payload)
            .map_err(|_| AudioError::BankFull)?;
        Ok(())
    }
}
