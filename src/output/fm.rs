#[cfg(feature = "fm")]
use crate::profile::markham::{self, clamp_frequency};

/// One control tick for frequency-modulated buzzer hardware (Markham-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FmTick {
    pub frequency_hz: u32,
    pub active: bool,
}

/// Maps mixed PCM to buzzer frequency (VCO / “PC speaker on piezo”).
#[derive(Debug, Clone, Copy)]
pub struct FmMapper {
    pub center_hz: u32,
    pub span_hz: u32,
    pub silence_threshold: i8,
}

impl FmMapper {
    pub const fn new(center_hz: u32, span_hz: u32, silence_threshold: i8) -> Self {
        Self {
            center_hz,
            span_hz,
            silence_threshold,
        }
    }

    pub const fn map_pcm(&self, pcm: i8) -> FmTick {
        let level = pcm.abs();
        if level <= self.silence_threshold {
            return FmTick {
                frequency_hz: self.center_hz,
                active: false,
            };
        }
        let hz = self.center_hz + (level as u32 * self.span_hz) / 127;
        FmTick {
            frequency_hz: clamp_frequency(hz),
            active: true,
        }
    }

    /// Use when the voice is a pure tone oscillator (Tier A `Tone`).
    pub const fn from_carrier(hz: u32, active: bool) -> FmTick {
        FmTick {
            frequency_hz: if active {
                clamp_frequency(hz)
            } else {
                hz
            },
            active,
        }
    }
}

#[cfg(feature = "fm")]
impl FmMapper {
    /// Markham firmware defaults (requires `fm` feature).
    pub const fn markham() -> Self {
        Self::new(
            markham::VCO_CENTER_HZ,
            markham::VCO_SPAN_HZ,
            markham::SILENCE_THRESHOLD,
        )
    }
}
