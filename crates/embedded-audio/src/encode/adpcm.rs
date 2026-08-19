//! IMA ADPCM encoder (host / `std` only). Output matches [`crate::decode::AdpcmStream`] layout.

extern crate alloc;

use alloc::vec::Vec;

const STEP_TABLE: [i16; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

const INDEX_TABLE: [i8; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

struct Encoder {
    predictor: i16,
    index: u8,
}

impl Encoder {
    fn new(predictor: i16, index: u8) -> Self {
        Self {
            predictor,
            index: index.min(88),
        }
    }

    fn encode_sample(&mut self, sample: i16) -> u8 {
        let step = STEP_TABLE[self.index as usize];
        let diff = sample.wrapping_sub(self.predictor);
        let sign = if diff < 0 { 8 } else { 0 };
        let mut delta = diff.abs();
        let mut nibble = sign;

        if delta >= step {
            nibble |= 4;
            delta -= step;
        }
        let mut temp = step >> 1;
        if delta >= temp {
            nibble |= 2;
            delta -= temp;
        }
        temp >>= 1;
        if delta >= temp {
            nibble |= 1;
        }

        let diffq = step >> 3;
        let mut est = diffq;
        if nibble & 4 != 0 {
            est += step;
        }
        if nibble & 2 != 0 {
            est += step >> 1;
        }
        if nibble & 1 != 0 {
            est += step >> 2;
        }
        if sign != 0 {
            self.predictor = self.predictor.saturating_sub(est);
        } else {
            self.predictor = self.predictor.saturating_add(est);
        }

        let idx = self.index as i32 + INDEX_TABLE[nibble as usize] as i32;
        self.index = idx.clamp(0, 88) as u8;
        nibble as u8
    }
}

/// Encode mono i16 PCM into ADPCM payload (4-byte header + packed nibbles).
pub fn encode_i16(pcm: &[i16]) -> Vec<u8> {
    let mut enc = Encoder::new(0, 0);
    let mut out = Vec::with_capacity(4 + pcm.len() / 2);
    out.extend_from_slice(&enc.predictor.to_le_bytes());
    out.push(enc.index);
    out.push(0);

    let mut high = true;
    let mut byte = 0u8;
    for &s in pcm {
        let n = enc.encode_sample(s);
        if high {
            byte = n << 4;
            high = false;
        } else {
            out.push(byte | n);
            high = true;
        }
    }
    if !high {
        out.push(byte);
    }
    out
}

/// Encode 8-bit unsigned PCM (0..255, center 128) to ADPCM payload.
pub fn encode_u8(pcm: &[u8]) -> Vec<u8> {
    let pcm16: Vec<i16> = pcm.iter().map(|&b| ((b as i16) - 128) << 8).collect();
    encode_i16(&pcm16)
}
