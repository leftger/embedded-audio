//! Defaults from the Markham main-controller firmware (`cew/markham`).
//!
//! Markham drives a piezo buzzer on **TIM3 / PE3** by varying **PWM frequency**
//! at ~50% duty, not by duty modulation at a fixed carrier.

/// Board init carrier for `SimplePwm::new` (runtime `set_frequency` overrides this).
pub const PWM_CARRIER_INIT_HZ: u32 = 2_000;

/// Software limits in `buzzer.rs`.
pub const MIN_FREQUENCY_HZ: u32 = 250;
pub const MAX_FREQUENCY_HZ: u32 = 8_000;

/// Startup / menu beep.
pub const DEFAULT_BEEP_HZ: u32 = 1_000;

/// First-boot verification double-beep.
pub const FIRST_BOOT_BEEP_HZ: u32 = 300;

/// How often the audio engine should call `tick_fm` when emulating samples on FM hardware.
pub const CONTROL_TICK_HZ: u32 = 1_000;

/// VCO center when mapping PCM → frequency (PC-speaker style on a buzzer).
pub const VCO_CENTER_HZ: u32 = 2_000;

/// ± deviation from center at full-scale PCM.
pub const VCO_SPAN_HZ: u32 = 3_000;

/// Samples below this level (after mix) are treated as silence.
pub const SILENCE_THRESHOLD: i8 = 8;

/// Clamp helper matching Markham `set_frequency`.
#[inline]
pub const fn clamp_frequency(hz: u32) -> u32 {
    let hz = if hz < MIN_FREQUENCY_HZ {
        MIN_FREQUENCY_HZ
    } else {
        hz
    };
    if hz > MAX_FREQUENCY_HZ {
        MAX_FREQUENCY_HZ
    } else {
        hz
    }
}
