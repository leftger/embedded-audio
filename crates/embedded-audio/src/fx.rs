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
