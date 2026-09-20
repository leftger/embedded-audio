//! ITU-T G.711 speech companding codecs (µ-law and A-law).
//!
//! G.711 encodes 13-14 bit dynamic range into 8-bit log-compressed audio samples.
//! Ubiquitous in telecommunications, IoT intercoms, walkie-talkies, and industrial voice prompts.

/// Decode a single ITU-T G.711 µ-law byte to linear 16-bit signed PCM (-32768..=32767).
#[inline]
pub fn g711_ulaw_decode(byte: u8) -> i16 {
    let flipped = !byte;
    let sign = flipped & 0x80;
    let exponent = ((flipped >> 4) & 0x07) as i32;
    let mantissa = (flipped & 0x0F) as i32;

    let sample = ((mantissa << 3) + 0x84) << exponent;
    let linear = sample - 0x84;

    if sign != 0 {
        linear.clamp(-32768, 32767) as i16
    } else {
        (-linear).clamp(-32768, 32767) as i16
    }
}

/// Encode linear 16-bit signed PCM to ITU-T G.711 µ-law byte.
#[inline]
pub fn g711_ulaw_encode(pcm: i16) -> u8 {
    const BIAS: i32 = 0x84;
    const CLIP: i32 = 32635;

    let mut sample = pcm as i32;
    let sign = if sample < 0 {
        sample = -sample;
        0x00
    } else {
        0x80
    };

    if sample > CLIP {
        sample = CLIP;
    }
    sample += BIAS;

    let mut exponent = 7;
    let mut mask = 0x4000;
    while exponent > 0 && (sample & mask) == 0 {
        exponent -= 1;
        mask >>= 1;
    }

    let mantissa = (sample >> (exponent + 3)) & 0x0F;
    let byte = (sign | (exponent << 4) | mantissa) as u8;
    !byte
}

/// Decode a single ITU-T G.711 A-law byte to linear 16-bit signed PCM (-32768..=32767).
#[inline]
pub fn g711_alaw_decode(byte: u8) -> i16 {
    let flipped = byte ^ 0x55;
    let sign = flipped & 0x80;
    let exponent = ((flipped >> 4) & 0x07) as i32;
    let mantissa = (flipped & 0x0F) as i32;

    let linear = if exponent == 0 {
        (mantissa << 4) + 8
    } else {
        ((mantissa << 4) + 0x108) << (exponent - 1)
    };

    if sign != 0 {
        linear.clamp(-32768, 32767) as i16
    } else {
        (-linear).clamp(-32768, 32767) as i16
    }
}

/// Encode linear 16-bit signed PCM to ITU-T G.711 A-law byte.
#[inline]
pub fn g711_alaw_encode(pcm: i16) -> u8 {
    const CLIP: i32 = 32767;

    let mut sample = pcm as i32;
    let sign = if sample < 0 {
        sample = -sample;
        0x00
    } else {
        0x80
    };

    if sample > CLIP {
        sample = CLIP;
    }

    let (exponent, mantissa) = if sample >= 256 {
        let mut exp = 7;
        let mut mask = 0x4000;
        while exp > 1 && (sample & mask) == 0 {
            exp -= 1;
            mask >>= 1;
        }
        (exp, (sample >> (exp + 3)) & 0x0F)
    } else {
        (0, (sample >> 4) & 0x0F)
    };

    let byte = (sign | (exponent << 4) | mantissa) as u8;
    byte ^ 0x55
}

/// ITU-T G.711 companding format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum G711Format {
    #[default]
    MuLaw,
    ALaw,
}

/// Streaming G.711 audio decoder with looping support.
#[derive(Debug, Clone, Copy)]
pub struct G711Stream<'a> {
    data: &'a [u8],
    cursor: usize,
    format: G711Format,
    looped: bool,
}

impl<'a> G711Stream<'a> {
    pub const fn new(data: &'a [u8], format: G711Format, looped: bool) -> Self {
        Self {
            data,
            cursor: 0,
            format,
            looped,
        }
    }

    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    pub fn is_done(&self) -> bool {
        self.cursor >= self.data.len() && !self.looped
    }

    /// Decode the next sample to signed 16-bit PCM.
    pub fn next_sample_i16(&mut self) -> Option<i16> {
        if self.data.is_empty() {
            return None;
        }
        if self.cursor >= self.data.len() {
            if self.looped {
                self.cursor = 0;
            } else {
                return None;
            }
        }
        let byte = self.data[self.cursor];
        self.cursor += 1;
        let s16 = match self.format {
            G711Format::MuLaw => g711_ulaw_decode(byte),
            G711Format::ALaw => g711_alaw_decode(byte),
        };
        Some(s16)
    }

    /// Decode the next sample scaled to signed 8-bit PCM.
    pub fn next_sample_i8(&mut self) -> Option<i8> {
        let s16 = self.next_sample_i16()?;
        Some((s16 >> 8) as i8)
    }
}
