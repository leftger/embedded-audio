//! Karplus-Strong plucked-string synthesis.
//!
//! Classic Karplus-Strong: a noise burst is written into a circular delay line sized to the
//! desired pitch period, then repeatedly read back through a variable-weight two-tap blend
//! (the "loop filter") that darkens and shortens the sound on every pass around the loop. No
//! sample memory is needed — inspired by DaisySP's `PhysicalModeling::String`, reworked here as
//! fixed-point `i8` PCM so it needs no floating-point math and stays in the always-on core
//! (unlike the `dsp`-feature synthesis in [`crate::drums`]).

use crate::fixed::clamp_sample;

/// A plucked string voice with a fixed-capacity delay line of `N` samples.
///
/// `N` bounds the lowest playable frequency: at a given `sample_rate_hz`, frequencies below
/// `sample_rate_hz / N` clamp to the longest period the buffer can hold. For a 16 kHz engine,
/// `N = 512` reaches down to ~31 Hz.
#[derive(Debug, Clone)]
pub struct KarplusPluck<const N: usize> {
    buffer: [i8; N],
    period: usize,
    pos: usize,
    rng_state: u32,
    /// 0 = darkest / fastest decay (full averaging), 255 = brightest / slowest decay (no filtering).
    brightness_q8: u8,
    /// Extra overall feedback attenuation per loop pass; 255 = lossless (rings until re-plucked).
    decay_q8: u8,
    samples_left: u32,
    active: bool,
}

impl<const N: usize> KarplusPluck<N> {
    /// Creates a stopped string voice. Call [`Self::pluck`] to start it ringing.
    pub const fn new() -> Self {
        Self {
            buffer: [0; N],
            period: if N < 2 { 2 } else { N },
            pos: 0,
            rng_state: 0x1234_5678,
            brightness_q8: 200,
            decay_q8: 255,
            samples_left: 0,
            active: false,
        }
    }

    /// Sets the tone's brightness/decay-speed balance. Lower values sound darker and decay faster.
    pub fn set_brightness_q8(&mut self, brightness_q8: u8) {
        self.brightness_q8 = brightness_q8;
    }

    /// Sets extra overall feedback loss per pass. `255` lets the string ring until re-plucked;
    /// lower values shorten the sustain independently of `brightness_q8`.
    pub fn set_decay_q8(&mut self, decay_q8: u8) {
        self.decay_q8 = decay_q8;
    }

    /// Excites the string at `freq_hz` with a pseudo-random noise burst of `amplitude`, ringing
    /// for `duration_ms` before auto-stopping (matching [`crate::synth::ToneVoice`]'s convention).
    pub fn pluck(&mut self, freq_hz: u32, amplitude: i8, duration_ms: u16, sample_rate_hz: u32) {
        self.period = if freq_hz == 0 || sample_rate_hz == 0 {
            self.buffer.len().max(2)
        } else {
            ((sample_rate_hz / freq_hz) as usize).clamp(2, self.buffer.len().max(2))
        };

        for i in 0..self.period {
            self.buffer[i] = self.next_noise_sample(amplitude);
        }
        self.pos = 0;
        self.samples_left = if duration_ms == 0 {
            u32::MAX
        } else {
            (duration_ms as u32 * sample_rate_hz) / 1000
        };
        self.active = true;
    }

    fn next_noise_sample(&mut self, amplitude: i8) -> i8 {
        // xorshift32: cheap, deterministic, no external dependency.
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        let noise = (self.rng_state >> 24) as i8;
        ((noise as i32 * amplitude as i32) / 127).clamp(-128, 127) as i8
    }

    /// Silences the string immediately.
    pub fn stop(&mut self) {
        self.active = false;
        self.samples_left = 0;
    }

    /// Whether the string is still ringing (has not reached `duration_ms` or been stopped).
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Advances the string by one sample, returning `None` once it has stopped.
    pub fn next_sample(&mut self) -> Option<i8> {
        if !self.active {
            return None;
        }
        if self.samples_left == 0 {
            self.active = false;
            return None;
        }
        if self.samples_left != u32::MAX {
            self.samples_left -= 1;
        }

        let period = self.period.max(2);
        let next_pos = if self.pos + 1 >= period { 0 } else { self.pos + 1 };
        let a = self.buffer[self.pos] as i32;
        let b = self.buffer[next_pos] as i32;
        let blended = a + ((b - a) * self.brightness_q8 as i32) / 256;
        let attenuated = (blended * self.decay_q8 as i32) / 256;
        self.buffer[self.pos] = clamp_sample(attenuated);
        self.pos = next_pos;
        Some(clamp_sample(a))
    }
}

impl<const N: usize> Default for KarplusPluck<N> {
    fn default() -> Self {
        Self::new()
    }
}
