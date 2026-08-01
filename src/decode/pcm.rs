use crate::fixed::lerp_i8;

/// Streaming 8-bit PCM (Tier B) with fractional speed resampling.
#[derive(Debug, Clone, Copy)]
pub struct Pcm8Stream<'a> {
    data: &'a [u8],
    phase_q16: u32,
    speed_q16: u32,
    looped: bool,
    is_signed: bool,
}

impl<'a> Pcm8Stream<'a> {
    pub fn new(data: &'a [u8], flags: u8) -> Self {
        Self::with_speed(data, flags, 65536)
    }

    pub fn with_speed(data: &'a [u8], flags: u8, speed_q16: u32) -> Self {
        Self {
            data,
            phase_q16: 0,
            speed_q16,
            looped: flags & crate::tier::flags::LOOP != 0,
            is_signed: flags & crate::tier::flags::SIGNED != 0,
        }
    }

    pub fn set_speed_q16(&mut self, speed_q16: u32) {
        self.speed_q16 = speed_q16;
    }

    pub fn speed_q16(&self) -> u32 {
        self.speed_q16
    }

    pub fn reset(&mut self) {
        self.phase_q16 = 0;
    }

    pub fn is_done(&self) -> bool {
        if self.data.is_empty() {
            return true;
        }
        let pos = (self.phase_q16 >> 16) as usize;
        !self.looped && pos >= self.data.len()
    }

    #[inline]
    fn get_sample(&self, idx: usize) -> i8 {
        if idx >= self.data.len() {
            0
        } else if self.is_signed {
            self.data[idx] as i8
        } else {
            self.data[idx].wrapping_sub(128) as i8
        }
    }

    pub fn next_sample(&mut self) -> Option<i8> {
        if self.data.is_empty() {
            return None;
        }

        let curr_idx = (self.phase_q16 >> 16) as usize;

        if curr_idx >= self.data.len() {
            if self.looped {
                let len = self.data.len();
                let wrap = (curr_idx % len) << 16;
                self.phase_q16 = (wrap as u32) | (self.phase_q16 & 0xFFFF);
            } else {
                return None;
            }
        }

        let idx = (self.phase_q16 >> 16) as usize;
        let frac = ((self.phase_q16 >> 8) & 0xFF) as u8;

        let a = self.get_sample(idx);
        let next_idx = if idx + 1 < self.data.len() {
            idx + 1
        } else if self.looped {
            0
        } else {
            idx
        };
        let b = self.get_sample(next_idx);

        let sample = lerp_i8(a, b, frac);
        self.phase_q16 = self.phase_q16.wrapping_add(self.speed_q16);

        Some(sample)
    }
}
