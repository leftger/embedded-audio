/// Tier C: packed 1-bit stream (MSB first). Each bit maps to ±127 PCM before duty.
#[derive(Debug, Clone, Copy)]
pub struct SigmaDeltaBitStream<'a> {
    data: &'a [u8],
    byte_index: usize,
    bit_mask: u8,
    looped: bool,
}

impl<'a> SigmaDeltaBitStream<'a> {
    pub fn new(data: &'a [u8], effect_flags: u8) -> Self {
        Self {
            data,
            byte_index: 0,
            bit_mask: 0x80,
            looped: effect_flags & crate::tier::flags::LOOP != 0,
        }
    }

    pub fn reset(&mut self) {
        self.byte_index = 0;
        self.bit_mask = 0x80;
    }

    pub fn is_done(&self) -> bool {
        !self.looped && self.byte_index >= self.data.len()
    }
}

impl<'a> SigmaDeltaBitStream<'a> {
    pub fn next_sample(&mut self) -> Option<i8> {
        if self.byte_index >= self.data.len() {
            if self.looped && !self.data.is_empty() {
                self.reset();
            } else {
                return None;
            }
        }
        let byte = self.data[self.byte_index];
        let high = (byte & self.bit_mask) != 0;
        self.bit_mask >>= 1;
        if self.bit_mask == 0 {
            self.bit_mask = 0x80;
            self.byte_index += 1;
        }
        Some(if high { 127 } else { -127 })
    }
}
