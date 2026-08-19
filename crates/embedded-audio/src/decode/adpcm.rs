/// IMA ADPCM step table (standard 89 values; we use first 89 indices).
const STEP_TABLE: [i16; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

const INDEX_TABLE: [i8; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

/// Stateful IMA ADPCM decoder.
#[derive(Debug, Clone, Copy)]
pub struct AdpcmDecoder {
    predictor: i16,
    index: u8,
    nibble_high: bool,
    current_byte: u8,
}

impl AdpcmDecoder {
    pub fn new(predictor: i16, index: u8) -> Self {
        Self {
            predictor,
            index: index.min(88),
            nibble_high: true,
            current_byte: 0,
        }
    }

    pub fn decode_nibble(&mut self, nibble: u8) -> i8 {
        let nibble = nibble & 0x0F;
        let step = STEP_TABLE[self.index as usize];
        let mut diff = step >> 3;
        if nibble & 4 != 0 {
            diff += step;
        }
        if nibble & 2 != 0 {
            diff += step >> 1;
        }
        if nibble & 1 != 0 {
            diff += step >> 2;
        }
        if nibble & 8 != 0 {
            self.predictor = (self.predictor as i32 - diff as i32).clamp(-32768, 32767) as i16;
        } else {
            self.predictor = (self.predictor as i32 + diff as i32).clamp(-32768, 32767) as i16;
        }
        let idx = self.index as i32 + INDEX_TABLE[nibble as usize] as i32;
        self.index = idx.clamp(0, 88) as u8;
        (self.predictor >> 8).clamp(-128, 127) as i8
    }

    fn next_from_bytes(&mut self, bytes: &mut &[u8]) -> Option<i8> {
        if self.nibble_high {
            let b = *bytes.first()?;
            self.current_byte = *bytes.first()?;
            *bytes = &bytes[1..];
            self.nibble_high = false;
            Some(self.decode_nibble(b >> 4))
        } else {
            self.nibble_high = true;
            Some(self.decode_nibble(self.current_byte))
        }
    }
}

/// Tier B ADPCM stream: 4-byte header (predictor le, index u8) + nibbles.
#[derive(Debug, Clone, Copy)]
pub struct AdpcmStream<'a> {
    payload: &'a [u8],
    bytes: &'a [u8],
    decoder: AdpcmDecoder,
    looped: bool,
}

impl<'a> AdpcmStream<'a> {
    pub fn new(payload: &'a [u8], flags: u8) -> Option<Self> {
        if payload.len() < 4 {
            return None;
        }
        let predictor = i16::from_le_bytes([payload[0], payload[1]]);
        let index = payload[2];
        let decoder = AdpcmDecoder::new(predictor, index);
        Some(Self {
            payload,
            bytes: &payload[4..],
            decoder,
            looped: flags & crate::tier::flags::LOOP != 0,
        })
    }

    fn rewind(&mut self) {
        if let Some(s) = Self::new(self.payload, crate::tier::flags::LOOP) {
            *self = s;
        }
    }

    pub fn is_done(&self) -> bool {
        self.bytes.is_empty() && !self.looped
    }
}

impl<'a> AdpcmStream<'a> {
    pub fn next_sample(&mut self) -> Option<i8> {
        if self.bytes.is_empty() {
            if self.looped {
                self.rewind();
            } else {
                return None;
            }
        }
        let mut slice = self.bytes;
        let sample = self.decoder.next_from_bytes(&mut slice)?;
        self.bytes = slice;
        Some(sample)
    }
}
