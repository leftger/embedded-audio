#[cfg(feature = "fm")]
pub mod fm;

#[cfg(feature = "fm")]
pub use fm::{FmMapper, FmTick};

use crate::fixed::clamp_sample;

/// How PCM is converted to a PWM compare value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DutyMode {
    /// Mid-scale duty plus linear scaling from PCM.
    #[default]
    Linear,
    /// First-order sigma-delta noise shaping before duty mapping.
    SigmaDelta,
}

/// First-order sigma-delta modulator (PCM → single-bit decision → duty).
#[derive(Debug, Clone, Copy, Default)]
pub struct SigmaDelta {
    integrator: i32,
}

impl SigmaDelta {
    pub const fn new() -> Self {
        Self { integrator: 0 }
    }

    pub fn reset(&mut self) {
        self.integrator = 0;
    }

    /// One modulator step; returns -127..=127 shaped PCM for duty mapping.
    pub fn shape(&mut self, pcm: i8) -> i8 {
        let input = (pcm as i32) << 8;
        self.integrator += input;
        let high = self.integrator >= 0;
        if high {
            self.integrator -= 65536;
        }
        if high { 127 } else { -127 }
    }
}

/// Map shaped PCM to PWM duty in `1..period-1`.
#[inline]
pub fn pcm_to_duty(pcm: i8, period: u16) -> u16 {
    let mid = period as i32 / 2;
    let swing = mid - 1;
    let offset = (pcm as i32 * swing) / 127;
    (mid + offset).clamp(1, period as i32 - 1) as u16
}

/// Full output path: optional ΣΔ then duty.
pub struct PwmMapper {
    pub mode: DutyMode,
    pub sigma_delta: SigmaDelta,
}

impl PwmMapper {
    pub const fn new(mode: DutyMode) -> Self {
        Self {
            mode,
            sigma_delta: SigmaDelta::new(),
        }
    }

    pub fn map(&mut self, pcm: i8, period: u16) -> u16 {
        let shaped = match self.mode {
            DutyMode::Linear => pcm,
            DutyMode::SigmaDelta => self.sigma_delta.shape(pcm),
        };
        pcm_to_duty(shaped, period)
    }

    pub fn reset(&mut self) {
        self.sigma_delta.reset();
    }
}

/// Soft limiter before mix bus.
#[inline]
pub fn limit_bus(sum: i32) -> i8 {
    clamp_sample(sum)
}
