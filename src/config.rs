use crate::output::DutyMode;

/// Recommended duty-modulation defaults for piezo / buzzer PWM audio.
pub const DEFAULT_SAMPLE_RATE_HZ: u32 = 16_000;

/// Fixed PWM carrier; must exceed ~2× the highest passed audio frequency.
pub const DEFAULT_PWM_CARRIER_HZ: u32 = 32_000;

/// Default timer period counts (ARR+1). Set from your timer setup:
/// `period ≈ timer_clk / carrier_hz`.
pub const DEFAULT_PWM_PERIOD: u16 = 1000;

/// Engine and bank builder configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioConfig {
    pub sample_rate_hz: u32,
    pub pwm_period: u16,
    pub duty_mode: DutyMode,
    pub master_gain_q8: u8,
}

impl AudioConfig {
    pub const fn new(sample_rate_hz: u32, pwm_period: u16, duty_mode: DutyMode) -> Self {
        Self {
            sample_rate_hz,
            pwm_period,
            duty_mode,
            master_gain_q8: 255,
        }
    }

    pub const fn default_duty() -> Self {
        Self::new(
            DEFAULT_SAMPLE_RATE_HZ,
            DEFAULT_PWM_PERIOD,
            DutyMode::SigmaDelta,
        )
    }

    pub const fn master_gain_q8(mut self, gain: u8) -> Self {
        self.master_gain_q8 = gain;
        self
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self::default_duty()
    }
}

/// Crossfade step in Q8 units so a fade completes in `duration_ms`.
#[inline]
pub const fn crossfade_step_q8(duration_ms: u16, sample_rate_hz: u32) -> u8 {
    if duration_ms == 0 || sample_rate_hz == 0 {
        return 255;
    }
    let samples = (duration_ms as u32 * sample_rate_hz) / 1000;
    if samples == 0 {
        return 255;
    }
    let step = (255u32 + samples - 1) / samples;
    if step > 255 { 255 } else { step as u8 }
}
