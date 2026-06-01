/// Streaming 8-bit PCM (Tier B).
#[derive(Debug, Clone, Copy)]
pub struct Pcm8Stream<'a> {
    data: &'a [u8],
    pos: usize,
    looped: bool,
}

impl<'a> Pcm8Stream<'a> {
    pub fn new(data: &'a [u8], flags: u8) -> Self {
        Self {
            data,
            pos: 0,
            looped: flags & crate::tier::flags::LOOP != 0,
        }
    }

    pub fn reset(&mut self) {
        self.pos = 0;
    }

    pub fn is_done(&self) -> bool {
        !self.looped && self.pos >= self.data.len()
    }
}

impl<'a> Pcm8Stream<'a> {
    pub fn next_sample(&mut self) -> Option<i8> {
        if self.pos >= self.data.len() {
            if self.looped && !self.data.is_empty() {
                self.pos = 0;
            } else {
                return None;
            }
        }
        let s = self.data[self.pos] as i8;
        self.pos += 1;
        Some(s)
    }
}
