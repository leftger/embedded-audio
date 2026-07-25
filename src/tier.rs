/// Effect payload type (matches bank directory `kind` byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EffectKind {
    /// Tier A: tone oscillator (`param0` = Hz, `param1` = duration ms).
    Tone = 0,
    /// Tier A: wavetable loop (`param0` = Hz).
    Wavetable = 1,
    /// Tier A: two-operator FM (`param0` = carrier Hz, `param1` = mod ratio ×100).
    Fm = 2,
    /// Tier B: raw 8-bit PCM.
    Pcm8 = 3,
    /// Tier B: IMA ADPCM (4-bit) with 4-byte header predictor/index.
    Adpcm = 4,
    /// Tier C: packed 1-bit sigma-delta bitstream (MSB first).
    SigmaDeltaBits = 5,
}

impl EffectKind {
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Tone),
            1 => Some(Self::Wavetable),
            2 => Some(Self::Fm),
            3 => Some(Self::Pcm8),
            4 => Some(Self::Adpcm),
            5 => Some(Self::SigmaDeltaBits),
            _ => None,
        }
    }
}

pub mod flags {
    pub const LOOP: u8 = 1 << 0;
    pub const ONE_SHOT: u8 = 1 << 1;
}
