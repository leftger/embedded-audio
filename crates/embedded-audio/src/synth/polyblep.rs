//! Anti-aliased band-limited oscillators using Polynomial Band-Limited Step (PolyBLEP).
//!
//! Adapted and optimized for embedded `#![no_std]` microcontrollers from open-source audio DSP research
//! (e.g. *Martin Finke*, *Andrew Jerrim's DSP-Testbench*, and *Matthijs Hollemans' synth-plugin-book*).
//!
//! Standard digital oscillators generate sharp edges that introduce strong aliasing at frequencies above
//! the Nyquist rate ($f_s / 2$). PolyBLEP corrects step discontinuities using a 2nd-order polynomial residual
//! evaluated only in the transition neighborhood ($|t| < \Delta t$), providing 30-40dB alias suppression
//! with minimal CPU cycles and zero heap allocations.

use core::f32::consts::PI;

#[cfg(feature = "std")]
#[inline(always)]
fn sin_f32(val: f32) -> f32 {
    val.sin()
}

#[cfg(not(feature = "std"))]
#[inline(always)]
fn sin_f32(val: f32) -> f32 {
    // Parabolic / Bhaskara approximation normalized to [-PI, PI] for pure no-std
    let two_pi = 2.0 * PI;
    let mut x = val % two_pi;
    if x < 0.0 {
        x += two_pi;
    }
    let (sign, y) = if x > PI { (-1.0, x - PI) } else { (1.0, x) };
    sign * (16.0 * y * (PI - y)) / (5.0 * PI * PI - 4.0 * y * (PI - y))
}

/// Waveform shape for the PolyBLEP oscillator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PolyBlepWaveform {
    /// Pure sine wave.
    #[default]
    Sine,
    /// Anti-aliased sawtooth wave (falling edge discontinuity).
    Sawtooth,
    /// Anti-aliased square wave with 50% duty cycle (rising and falling edge PolyBLEP).
    Square,
    /// Anti-aliased triangle wave using PolyBLAMP integrated corrections.
    Triangle,
}

/// Computes the 2nd-order polynomial residual for a unit step discontinuity at phase `t`
/// within normalized phase increment `dt`.
#[inline(always)]
pub fn poly_blep(mut t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    // 0 <= t < dt
    if t < dt {
        t /= dt;
        // 2*t - t^2 - 1.0
        t + t - t * t - 1.0
    }
    // 1 - dt < t <= 1
    else if t > 1.0 - dt {
        t = (t - 1.0) / dt;
        // t^2 + 2*t + 1.0
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Computes the integrated PolyBLAMP correction for slope discontinuities in triangle waves.
#[inline(always)]
pub fn poly_blamp(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if t < dt {
        let u = t / dt - 1.0;
        -(u * u * u) / 3.0 * (4.0 * dt)
    } else if t > 1.0 - dt {
        let u = (t - 1.0) / dt + 1.0;
        (u * u * u) / 3.0 * (4.0 * dt)
    } else {
        0.0
    }
}

/// Band-limited oscillator running in floating-point for high-fidelity tone synthesis.
#[derive(Debug, Clone, Copy)]
pub struct PolyBlepOscillator {
    phase: f32,
    phase_inc: f32,
    waveform: PolyBlepWaveform,
    freq_hz: u32,
    sample_rate_hz: u32,
}

impl Default for PolyBlepOscillator {
    fn default() -> Self {
        Self::new(440, 48_000, PolyBlepWaveform::Sine)
    }
}

impl PolyBlepOscillator {
    /// Creates a new PolyBLEP oscillator at the specified frequency and sample rate.
    pub fn new(freq_hz: u32, sample_rate_hz: u32, waveform: PolyBlepWaveform) -> Self {
        let mut osc = Self {
            phase: 0.0,
            phase_inc: 0.0,
            waveform,
            freq_hz,
            sample_rate_hz: sample_rate_hz.max(1),
        };
        osc.update_phase_inc();
        osc
    }

    /// Sets the oscillator frequency in Hz.
    pub fn set_frequency(&mut self, freq_hz: u32) {
        self.freq_hz = freq_hz;
        self.update_phase_inc();
    }

    /// Sets the sample rate in Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: u32) {
        self.sample_rate_hz = sample_rate_hz.max(1);
        self.update_phase_inc();
    }

    /// Sets the waveform shape.
    pub fn set_waveform(&mut self, waveform: PolyBlepWaveform) {
        self.waveform = waveform;
    }

    /// Returns current frequency in Hz.
    pub const fn frequency(&self) -> u32 {
        self.freq_hz
    }

    /// Resets oscillator phase to zero.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    fn update_phase_inc(&mut self) {
        self.phase_inc = (self.freq_hz as f32 / self.sample_rate_hz as f32).clamp(0.0, 0.499);
    }

    /// Advances the oscillator by one sample and returns a sample in `[-1.0, 1.0]`.
    pub fn next_sample(&mut self) -> f32 {
        let t = self.phase;
        let dt = self.phase_inc;

        let sample = match self.waveform {
            PolyBlepWaveform::Sine => sin_f32(t * 2.0 * PI),
            PolyBlepWaveform::Sawtooth => {
                // Naive saw from 1.0 down to -1.0
                let mut s = 1.0 - 2.0 * t;
                // Add PolyBLEP correction at t = 0
                s -= poly_blep(t, dt);
                s
            }
            PolyBlepWaveform::Square => {
                // Naive square: 1.0 for first half, -1.0 for second half
                let mut s = if t < 0.5 { 1.0 } else { -1.0 };
                // Add PolyBLEP at rising edge t = 0
                s += poly_blep(t, dt);
                // Subtract PolyBLEP at falling edge t = 0.5
                let t_half = if t >= 0.5 { t - 0.5 } else { t + 0.5 };
                s -= poly_blep(t_half, dt);
                s
            }
            PolyBlepWaveform::Triangle => {
                // Naive triangle
                let mut s = if t < 0.5 {
                    4.0 * t - 1.0
                } else {
                    3.0 - 4.0 * t
                };
                // PolyBLAMP corrections at slope changes
                s -= poly_blamp(t, dt);
                let t_half = if t >= 0.5 { t - 0.5 } else { t + 0.5 };
                s += poly_blamp(t_half, dt);
                s
            }
        };

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample.clamp(-1.0, 1.0)
    }
}

/// Voice wrapper for `PolyBlepOscillator` integrating into the `embedded-audio` mixer.
#[derive(Debug, Clone, Copy)]
pub struct PolyBlepVoice {
    osc: PolyBlepOscillator,
    samples_left: u32,
    active: bool,
}

impl Default for PolyBlepVoice {
    fn default() -> Self {
        Self::new()
    }
}

impl PolyBlepVoice {
    /// Creates a silent, inactive PolyBLEP voice.
    pub const fn new() -> Self {
        Self {
            osc: PolyBlepOscillator {
                phase: 0.0,
                phase_inc: 0.0,
                waveform: PolyBlepWaveform::Sine,
                freq_hz: 440,
                sample_rate_hz: 48_000,
            },
            samples_left: 0,
            active: false,
        }
    }

    /// Starts tone playback with specified frequency, duration, and waveform.
    pub fn start(
        &mut self,
        freq_hz: u32,
        duration_ms: u16,
        waveform: PolyBlepWaveform,
        sample_rate_hz: u32,
    ) {
        self.osc = PolyBlepOscillator::new(freq_hz, sample_rate_hz, waveform);
        self.samples_left = if duration_ms == 0 {
            u32::MAX
        } else {
            (duration_ms as u32 * sample_rate_hz) / 1000
        };
        self.active = true;
    }

    /// Immediately stops the voice.
    pub fn stop(&mut self) {
        self.active = false;
        self.samples_left = 0;
    }

    /// Returns whether the voice is actively producing audio.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Carrier frequency in Hz.
    pub const fn carrier_hz(&self) -> u32 {
        if self.active { self.osc.frequency() } else { 0 }
    }

    /// Generates next floating-point sample in `[-1.0, 1.0]`.
    pub fn next_sample_f32(&mut self) -> Option<f32> {
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

        Some(self.osc.next_sample())
    }

    /// Generates next 8-bit signed sample for the embedded mixer (`[-127, 127]`).
    pub fn next_sample(&mut self) -> Option<i8> {
        let f = self.next_sample_f32()?;
        let i = (f * 127.0) as i32;
        Some(i.clamp(-127, 127) as i8)
    }
}
