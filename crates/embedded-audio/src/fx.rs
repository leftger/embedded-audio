//! Lightweight per-sample effects ported from DaisySP's `Effects` module: waveshaping
//! distortion, wavefolding, and tremolo. All three run on `i8` PCM with plain arithmetic
//! (no `libm`/`std`), so they're part of the always-on core rather than the `dsp` feature.

use crate::fixed::{Phase, apply_gain_q8, hz_to_phase_inc, phase_index, sin_table};

fn soft_limit(x: f32) -> f32 {
    x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
}

fn soft_clip(x: f32) -> f32 {
    if x < -3.0 {
        -1.0
    } else if x > 3.0 {
        1.0
    } else {
        soft_limit(x)
    }
}

fn i8_to_f32(sample: i8) -> f32 {
    sample as f32 / 127.0
}

fn f32_to_i8(sample: f32) -> i8 {
    (sample * 127.0).clamp(-128.0, 127.0) as i8
}

/// Waveshaping distortion/overdrive, ported from `daisysp::Overdrive`.
#[derive(Debug, Clone, Copy)]
pub struct Overdrive {
    pre_gain: f32,
    post_gain: f32,
}

impl Overdrive {
    /// Creates an overdrive with `drive` in `0.0..=1.0` (`1.0` = max fuzz). Note this mirrors
    /// DaisySP exactly: `drive = 0.0` drives the pre-gain to zero, muting the signal rather than
    /// passing it through clean — pick a small nonzero drive (e.g. `0.1`) for a mild effect.
    pub fn new(drive: f32) -> Self {
        let mut od = Self {
            pre_gain: 0.0,
            post_gain: 1.0,
        };
        od.set_drive(drive);
        od
    }

    /// Sets the drive amount, clamped to `0.0..=1.0`.
    pub fn set_drive(&mut self, drive: f32) {
        let drive = 2.0 * drive.clamp(0.0, 1.0);
        let drive_2 = drive * drive;
        let pre_gain_a = drive * 0.5;
        let pre_gain_b = drive_2 * drive_2 * drive * 24.0;
        self.pre_gain = pre_gain_a + (pre_gain_b - pre_gain_a) * drive_2;

        let drive_squashed = drive * (2.0 - drive);
        self.post_gain = 1.0 / soft_clip(0.33 + drive_squashed * (self.pre_gain - 0.33));
    }

    /// Processes one PCM sample.
    pub fn process(&self, input: i8) -> i8 {
        let pre = self.pre_gain * i8_to_f32(input);
        f32_to_i8(soft_clip(pre) * self.post_gain)
    }
}

fn floor_f32(x: f32) -> f32 {
    let truncated = x as i32 as f32;
    if truncated > x {
        truncated - 1.0
    } else {
        truncated
    }
}

/// Wavefolder, ported from `daisysp::Wavefolder`. Input magnitude beyond `1.0` (post-gain)
/// folds back on itself instead of clipping.
#[derive(Debug, Clone, Copy)]
pub struct Wavefolder {
    gain: f32,
    offset: f32,
}

impl Wavefolder {
    /// Creates a wavefolder at unity gain with no DC offset.
    pub const fn new() -> Self {
        Self {
            gain: 1.0,
            offset: 0.0,
        }
    }

    /// Sets the input gain. Negative values fold through zero.
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    /// Sets a pre-gain DC offset for asymmetrical folding.
    pub fn set_offset(&mut self, offset: f32) {
        self.offset = offset;
    }

    /// Processes one PCM sample.
    pub fn process(&self, input: i8) -> i8 {
        let x = (i8_to_f32(input) + self.offset) * self.gain;
        let fold_count = floor_f32((x + 1.0) * 0.5);
        let sign = if (fold_count as i64) % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        f32_to_i8(sign * (x - 2.0 * fold_count))
    }
}

impl Default for Wavefolder {
    fn default() -> Self {
        Self::new()
    }
}

/// Amplitude tremolo driven by the crate's built-in sine wavetable, ported from
/// `daisysp::Tremolo`.
#[derive(Debug, Clone, Copy)]
pub struct Tremolo {
    phase: Phase,
    phase_inc: u32,
    half_depth_q8: u8,
}

impl Tremolo {
    /// Creates a tremolo at 1 Hz, full depth (call [`Self::set_freq`]/[`Self::set_depth_q8`] to taste).
    pub const fn new() -> Self {
        Self {
            phase: 0,
            phase_inc: 0,
            half_depth_q8: 127,
        }
    }

    /// Sets the LFO rate in Hz.
    pub fn set_freq(&mut self, freq_hz: u32, sample_rate_hz: u32) {
        self.phase_inc = hz_to_phase_inc(freq_hz, sample_rate_hz);
    }

    /// Sets how much to modulate volume, `0` (no effect) to `255` (full tremolo, silent at trough).
    pub fn set_depth_q8(&mut self, depth_q8: u8) {
        self.half_depth_q8 = depth_q8 / 2;
    }

    /// Processes one PCM sample.
    pub fn process(&mut self, input: i8) -> i8 {
        let lfo = sin_table(phase_index(self.phase)) as i32;
        self.phase = self.phase.wrapping_add(self.phase_inc);

        let half_depth = self.half_depth_q8 as i32;
        let dc = 255 - half_depth;
        let gain_q8 = (dc + (lfo * half_depth) / 127).clamp(0, 255) as u8;
        apply_gain_q8(input, gain_q8)
    }
}

impl Default for Tremolo {
    fn default() -> Self {
        Self::new()
    }
}

/// Zero-allocation circular buffer delay line with feedback, one-pole damping lowpass filter, and wet/dry mix.
///
/// `SAMPLES` specifies the buffer capacity in samples.
/// E.g. at 16 kHz sample rate, `SAMPLES = 4000` provides up to 250 ms of delay.
#[derive(Debug, Clone)]
pub struct DelayLine<const SAMPLES: usize> {
    buffer: [i8; SAMPLES],
    write_pos: usize,
    delay_samples: usize,
    feedback_q8: u8,
    damping_q8: u8,
    filter_state: i32,
    wet_q8: u8,
    dry_q8: u8,
}

impl<const SAMPLES: usize> DelayLine<SAMPLES> {
    /// Creates a delay line with specified initial delay in samples.
    pub const fn new(delay_samples: usize) -> Self {
        let delay = if delay_samples >= SAMPLES {
            if SAMPLES == 0 { 0 } else { SAMPLES - 1 }
        } else {
            delay_samples
        };
        Self {
            buffer: [0; SAMPLES],
            write_pos: 0,
            delay_samples: delay,
            feedback_q8: 128,
            damping_q8: 64,
            filter_state: 0,
            wet_q8: 128,
            dry_q8: 255,
        }
    }

    /// Set delay length in samples, clamped to `0..SAMPLES`.
    pub fn set_delay(&mut self, samples: usize) {
        self.delay_samples = if SAMPLES == 0 {
            0
        } else {
            samples.min(SAMPLES - 1)
        };
    }

    /// Delay length in samples.
    pub const fn delay(&self) -> usize {
        self.delay_samples
    }

    /// Set feedback gain `0..=255` (0 = no echo repeats, 255 = sustained self-oscillation).
    pub fn set_feedback_q8(&mut self, fb_q8: u8) {
        self.feedback_q8 = fb_q8;
    }

    /// Set damping filter `0..=255` (0 = bright reflections, 255 = heavily darkened repeats).
    pub fn set_damping_q8(&mut self, damping_q8: u8) {
        self.damping_q8 = damping_q8;
    }

    /// Set wet and dry mix levels in Q8.
    pub fn set_mix_q8(&mut self, wet_q8: u8, dry_q8: u8) {
        self.wet_q8 = wet_q8;
        self.dry_q8 = dry_q8;
    }

    /// Reset internal delay buffer and filter state to silence.
    pub fn reset(&mut self) {
        self.buffer = [0; SAMPLES];
        self.write_pos = 0;
        self.filter_state = 0;
    }

    /// Process one PCM sample through the delay line.
    pub fn process(&mut self, input: i8) -> i8 {
        if SAMPLES == 0 {
            return input;
        }

        let read_pos = (self.write_pos + SAMPLES - self.delay_samples) % SAMPLES;
        let delayed = self.buffer[read_pos];

        // One-pole lowpass filter for damping in feedback loop
        let alpha = 255 - self.damping_q8 as i32;
        self.filter_state += ((delayed as i32 - self.filter_state) * alpha) / 256;
        let filtered = self.filter_state;

        // Feedback + input
        let feedback_sample = (filtered * self.feedback_q8 as i32) / 256;
        let to_buffer = crate::fixed::soft_limit_i8(input as i32 + feedback_sample);
        self.buffer[self.write_pos] = to_buffer;
        self.write_pos = (self.write_pos + 1) % SAMPLES;

        // Wet/dry mix
        let out =
            ((input as i32 * self.dry_q8 as i32) + (delayed as i32 * self.wet_q8 as i32)) / 256;
        crate::fixed::soft_limit_i8(out)
    }
}

/// Ramp state for [`AntiPopRamp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RampState {
    Muted,
    RampingUp,
    Active,
    RampingDown,
}

/// Anti-pop soft ramp generator to eliminate startup and shutdown speaker pops/clicks.
///
/// Smoothly ramps gain or DC bias over a configurable number of samples (e.g. 5–20 ms)
/// to prevent mechanical speaker "thumps" and power-down amplifier clicks.
#[derive(Debug, Clone, Copy)]
pub struct AntiPopRamp {
    state: RampState,
    step: u32,
    total_steps: u32,
}

impl AntiPopRamp {
    /// Create a ramp initialized to `Muted` state with specified duration in samples.
    pub const fn new(ramp_samples: u32) -> Self {
        Self {
            state: RampState::Muted,
            step: 0,
            total_steps: if ramp_samples == 0 { 1 } else { ramp_samples },
        }
    }

    /// Trigger smooth ramp-up to `Active`.
    pub fn ramp_up(&mut self) {
        self.state = RampState::RampingUp;
    }

    /// Trigger smooth ramp-down to `Muted`.
    pub fn ramp_down(&mut self) {
        self.state = RampState::RampingDown;
    }

    /// Current ramp lifecycle state.
    pub const fn state(&self) -> RampState {
        self.state
    }

    /// True if fully ramped up to `Active`.
    pub const fn is_active(&self) -> bool {
        matches!(self.state, RampState::Active)
    }

    /// True if fully ramped down to `Muted`.
    pub const fn is_muted(&self) -> bool {
        matches!(self.state, RampState::Muted)
    }

    /// Advances the ramp state by one sample tick and returns the current Q8 gain factor (0..=255).
    pub fn tick_gain_q8(&mut self) -> u8 {
        match self.state {
            RampState::Muted => 0,
            RampState::Active => 255,
            RampState::RampingUp => {
                self.step = self.step.saturating_add(1);
                if self.step >= self.total_steps {
                    self.step = self.total_steps;
                    self.state = RampState::Active;
                    255
                } else {
                    ((self.step as u64 * 255) / self.total_steps as u64) as u8
                }
            }
            RampState::RampingDown => {
                self.step = self.step.saturating_sub(1);
                if self.step == 0 {
                    self.state = RampState::Muted;
                    0
                } else {
                    ((self.step as u64 * 255) / self.total_steps as u64) as u8
                }
            }
        }
    }

    /// Apply soft ramping to a PCM8 sample.
    pub fn apply(&mut self, sample: i8) -> i8 {
        let gain = self.tick_gain_q8();
        crate::fixed::apply_gain_q8(sample, gain)
    }

    /// Softly ramp a PWM duty cycle from 0 to its target mid-scale idle duty (anti-click on power up).
    pub fn apply_duty(&mut self, target_duty: u16) -> u16 {
        let gain = self.tick_gain_q8() as u32;
        ((target_duty as u32 * gain) / 255) as u16
    }
}
