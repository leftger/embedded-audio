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
    /// Second-order MASH/error-diffusion noise shaping for higher frequency attenuation.
    SigmaDelta2ndOrder,
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

/// Second-order sigma-delta modulator (MASH 1-1 / dual error integrator).
#[derive(Debug, Clone, Copy, Default)]
pub struct SigmaDelta2ndOrder {
    e1: i32,
    e2: i32,
}

impl SigmaDelta2ndOrder {
    pub const fn new() -> Self {
        Self { e1: 0, e2: 0 }
    }

    pub fn reset(&mut self) {
        self.e1 = 0;
        self.e2 = 0;
    }

    pub fn shape(&mut self, pcm: i8) -> i8 {
        let input = (pcm as i32) << 8;
        self.e1 += input;
        let y1 = if self.e1 >= 0 { 32767 } else { -32768 };
        self.e1 -= y1;

        self.e2 += self.e1;
        let y2 = if self.e2 >= 0 { 32767 } else { -32768 };
        self.e2 -= y2;

        let out = (y1 + y2) >> 9;
        out.clamp(-127, 127) as i8
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
    pub sigma_delta_2nd: SigmaDelta2ndOrder,
}

impl PwmMapper {
    pub const fn new(mode: DutyMode) -> Self {
        Self {
            mode,
            sigma_delta: SigmaDelta::new(),
            sigma_delta_2nd: SigmaDelta2ndOrder::new(),
        }
    }

    pub fn map(&mut self, pcm: i8, period: u16) -> u16 {
        let shaped = match self.mode {
            DutyMode::Linear => pcm,
            DutyMode::SigmaDelta => self.sigma_delta.shape(pcm),
            DutyMode::SigmaDelta2ndOrder => self.sigma_delta_2nd.shape(pcm),
        };
        pcm_to_duty(shaped, period)
    }

    pub fn reset(&mut self) {
        self.sigma_delta.reset();
        self.sigma_delta_2nd.reset();
    }
}

/// Soft limiter before mix bus.
#[inline]
pub fn limit_bus(sum: i32) -> i8 {
    clamp_sample(sum)
}

/// Convert signed 8-bit PCM (-128..=127) to signed 16-bit PCM (-32768..=32767).
#[inline]
pub fn pcm_to_i16(pcm: i8) -> i16 {
    (pcm as i16) << 8
}

/// Convert signed 8-bit PCM (-128..=127) to signed 32-bit PCM (24-bit aligned).
#[inline]
pub fn pcm_to_i32(pcm: i8) -> i32 {
    (pcm as i32) << 24
}

/// Convert signed 8-bit PCM (-128..=127) to unsigned 8-bit DAC sample (0..=255).
#[inline]
pub fn pcm_to_dac_u8(pcm: i8) -> u8 {
    (pcm as i16 + 128).clamp(0, 255) as u8
}

/// Convert signed 8-bit PCM (-128..=127) to unsigned 12-bit DAC sample (0..=4095, e.g. STM32 DAC1).
#[inline]
pub fn pcm_to_dac_u12(pcm: i8) -> u16 {
    ((pcm as i32 + 128) * 4095 / 255).clamp(0, 4095) as u16
}

/// Convert signed 8-bit PCM (-128..=127) to unsigned 16-bit DAC sample (0..=65535).
#[inline]
pub fn pcm_to_dac_u16(pcm: i8) -> u16 {
    ((pcm as i32 + 128) << 8).clamp(0, 65535) as u16
}
