//! Optional Digital Signal Processing (DSP) integrations powered by `embedded-dsp`.
//!
//! This module provides real-time audio manipulation and analysis primitives for embedded systems:
//! - **Biquad Filters**: Highpass, Lowpass, Bandpass, Notch biquad filters for audio filtering and tone shaping.
//! - **Spectrum Analysis**: Windowed Real FFT (RFFT) spectrum analysis for pitch detection and spectral magnitude visualization.
//! - **Audio Metering**: RMS amplitude, peak detection, signal power, and variance metering.
//! - **LMS Adaptive Filtering**: Real-time noise cancellation and system identification.

#[allow(unused_imports)]
use embedded_dsp::FloatMath;
use embedded_dsp::{
    BiquadCascadeInstanceF32, LmsInstanceF32, apply_window_f32, biquad_cascade_df1_f32,
    blackman_f32, flattop_f32, hamming_f32, hanning_f32, lms_f32, mean_f32, power_f32, rfft_f32,
    rms_f32, var_f32,
};

/// Biquad audio filter for real-time sample-by-sample or block filtering.
#[derive(Clone)]
pub struct BiquadAudioFilter {
    coeffs: [f32; 5], // [b0, b1, b2, a1, a2]
    state: [f32; 4],  // [x[n-1], x[n-2], y[n-1], y[n-2]]
}

impl BiquadAudioFilter {
    /// Create a custom biquad filter given 5 normalized coefficients `[b0, b1, b2, a1, a2]`.
    pub fn new(b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) -> Self {
        Self {
            coeffs: [b0, b1, b2, a1, a2],
            state: [0.0; 4],
        }
    }

    /// Design a 2nd-order Lowpass Biquad filter.
    pub fn lowpass(cutoff_hz: f32, sample_rate_hz: f32, q: f32) -> Self {
        let omega = 2.0 * core::f32::consts::PI * cutoff_hz / sample_rate_hz;
        let alpha = omega.sin() / (2.0 * q);
        let cos_w = omega.cos();

        let b0 = (1.0 - cos_w) / 2.0;
        let b1 = 1.0 - cos_w;
        let b2 = (1.0 - cos_w) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w;
        let a2 = 1.0 - alpha;

        Self::new(b0 / a0, b1 / a0, b2 / a0, -a1 / a0, -a2 / a0)
    }

    /// Design a 2nd-order Highpass Biquad filter.
    pub fn highpass(cutoff_hz: f32, sample_rate_hz: f32, q: f32) -> Self {
        let omega = 2.0 * core::f32::consts::PI * cutoff_hz / sample_rate_hz;
        let alpha = omega.sin() / (2.0 * q);
        let cos_w = omega.cos();

        let b0 = (1.0 + cos_w) / 2.0;
        let b1 = -(1.0 + cos_w);
        let b2 = (1.0 + cos_w) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w;
        let a2 = 1.0 - alpha;

        Self::new(b0 / a0, b1 / a0, b2 / a0, -a1 / a0, -a2 / a0)
    }

    /// Design a 2nd-order Bandpass Biquad filter (constant peak gain).
    pub fn bandpass(cutoff_hz: f32, sample_rate_hz: f32, q: f32) -> Self {
        let omega = 2.0 * core::f32::consts::PI * cutoff_hz / sample_rate_hz;
        let alpha = omega.sin() / (2.0 * q);
        let cos_w = omega.cos();

        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w;
        let a2 = 1.0 - alpha;

        Self::new(b0 / a0, b1 / a0, b2 / a0, -a1 / a0, -a2 / a0)
    }

    /// Design a 2nd-order Notch (Band-Stop) Biquad filter.
    pub fn notch(cutoff_hz: f32, sample_rate_hz: f32, q: f32) -> Self {
        let omega = 2.0 * core::f32::consts::PI * cutoff_hz / sample_rate_hz;
        let alpha = omega.sin() / (2.0 * q);
        let cos_w = omega.cos();

        let b0 = 1.0;
        let b1 = -2.0 * cos_w;
        let b2 = 1.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w;
        let a2 = 1.0 - alpha;

        Self::new(b0 / a0, b1 / a0, b2 / a0, -a1 / a0, -a2 / a0)
    }

    /// Reset internal delay line state to 0.
    pub fn reset(&mut self) {
        self.state.fill(0.0);
    }

    /// Process a single floating-point sample in range `[-1.0, 1.0]`.
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let mut inst = BiquadCascadeInstanceF32 {
            num_stages: 1,
            coeffs: &self.coeffs,
            state: &mut self.state,
        };
        let src = [input];
        let mut dst = [0.0];
        biquad_cascade_df1_f32(&mut inst, &src, &mut dst);
        dst[0]
    }

    /// Process a PCM8 sample (`i8`).
    pub fn process_pcm8(&mut self, input: i8) -> i8 {
        let in_f32 = input as f32 / 128.0;
        let out_f32 = self.process_sample(in_f32);
        (out_f32 * 127.0).clamp(-128.0, 127.0) as i8
    }

    /// Process a block of floating-point audio samples in place.
    pub fn process_buffer(&mut self, samples: &mut [f32]) {
        let mut inst = BiquadCascadeInstanceF32 {
            num_stages: 1,
            coeffs: &self.coeffs,
            state: &mut self.state,
        };
        for sample in samples.iter_mut() {
            let src = [*sample];
            let mut dst = [0.0];
            biquad_cascade_df1_f32(&mut inst, &src, &mut dst);
            *sample = dst[0];
        }
    }
}

/// Window function types for spectral analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    Rectangular,
    Hanning,
    Hamming,
    Blackman,
    FlatTop,
}

/// FFT-based audio spectrum analyzer for real-time embedded feature extraction.
pub struct AudioSpectrumAnalyzer;

impl AudioSpectrumAnalyzer {
    /// Compute the magnitude spectrum of real audio samples `src`.
    ///
    /// `src` length must match `n`. `dst_mag` receives `n / 2` magnitude bins.
    pub fn analyze_spectrum(src: &[f32], window_type: WindowType, dst_mag: &mut [f32]) {
        let n = src.len();
        if n < 2 || (n & (n - 1)) != 0 || dst_mag.len() < n / 2 {
            return;
        }

        let mut win_buf = [0.0f32; 1024];
        let mut sample_buf = [0.0f32; 1024];

        if n > 1024 {
            return;
        }

        sample_buf[..n].copy_from_slice(&src[..n]);

        match window_type {
            WindowType::Rectangular => {}
            WindowType::Hanning => {
                hanning_f32(&mut win_buf[..n]);
                apply_window_f32(&mut sample_buf[..n], &win_buf[..n]);
            }
            WindowType::Hamming => {
                hamming_f32(&mut win_buf[..n]);
                apply_window_f32(&mut sample_buf[..n], &win_buf[..n]);
            }
            WindowType::Blackman => {
                blackman_f32(&mut win_buf[..n]);
                apply_window_f32(&mut sample_buf[..n], &win_buf[..n]);
            }
            WindowType::FlatTop => {
                flattop_f32(&mut win_buf[..n]);
                apply_window_f32(&mut sample_buf[..n], &win_buf[..n]);
            }
        }

        let mut fft_out = [0.0f32; 2048];
        rfft_f32(&sample_buf[..n], &mut fft_out[..2 * n], n, 0);

        for k in 0..(n / 2) {
            let re = fft_out[2 * k];
            let im = fft_out[2 * k + 1];
            dst_mag[k] = (re * re + im * im).sqrt();
        }
    }

    /// Estimate dominant (peak) frequency in Hz and its magnitude.
    /// Returns `(frequency_hz, peak_magnitude)`.
    pub fn find_peak_frequency(
        src: &[f32],
        sample_rate_hz: f32,
        window_type: WindowType,
    ) -> (f32, f32) {
        let n = src.len();
        if n < 4 || (n & (n - 1)) != 0 {
            return (0.0, 0.0);
        }

        let num_bins = n / 2;
        let mut mag_buf = [0.0f32; 512];
        if num_bins > mag_buf.len() {
            return (0.0, 0.0);
        }

        Self::analyze_spectrum(src, window_type, &mut mag_buf[..num_bins]);

        let mut max_mag = 0.0f32;
        let mut max_bin = 0;

        for (k, &mag) in mag_buf[..num_bins].iter().enumerate().skip(1) {
            if mag > max_mag {
                max_mag = mag;
                max_bin = k;
            }
        }

        let bin_width = sample_rate_hz / (n as f32);
        let freq = (max_bin as f32) * bin_width;

        (freq, max_mag)
    }
}

/// Statistics metrics for an audio frame.
#[derive(Debug, Clone, Copy)]
pub struct AudioStats {
    pub rms: f32,
    pub peak: f32,
    pub mean: f32,
    pub power: f32,
    pub variance: f32,
}

/// Audio signal statistics and metering.
pub struct AudioMeter;

impl AudioMeter {
    /// Calculate RMS, peak, mean, power, and variance for a slice of float audio samples.
    pub fn measure(samples: &[f32]) -> AudioStats {
        if samples.is_empty() {
            return AudioStats {
                rms: 0.0,
                peak: 0.0,
                mean: 0.0,
                power: 0.0,
                variance: 0.0,
            };
        }

        let mut mean = 0.0f32;
        let mut rms = 0.0f32;
        let mut power = 0.0f32;
        let mut variance = 0.0f32;

        let _ = mean_f32(samples, &mut mean);
        let _ = rms_f32(samples, &mut rms);
        let _ = power_f32(samples, &mut power);
        let _ = var_f32(samples, &mut variance);

        let mut peak = 0.0f32;
        for &s in samples {
            let abs_s = s.abs();
            if abs_s > peak {
                peak = abs_s;
            }
        }

        AudioStats {
            rms,
            peak,
            mean,
            power,
            variance,
        }
    }
}

/// Adaptive LMS filter for noise reduction and system identification.
pub struct AudioLmsFilter<'a> {
    inst: LmsInstanceF32<'a>,
}

impl<'a> AudioLmsFilter<'a> {
    /// Initialize an LMS filter with specified number of taps, coefficient storage, state buffer, and step size `mu`.
    pub fn new(num_taps: u16, coeffs: &'a mut [f32], state: &'a mut [f32], mu: f32) -> Self {
        let inst = LmsInstanceF32::init(num_taps, coeffs, state, mu);
        Self { inst }
    }

    /// Process input signal and reference signal blocks.
    /// Writes output signal into `out` and error signal into `err`.
    pub fn process(&mut self, src: &[f32], ref_signal: &[f32], out: &mut [f32], err: &mut [f32]) {
        lms_f32(&mut self.inst, src, ref_signal, out, err);
    }
}
