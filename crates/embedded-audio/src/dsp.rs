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

// Re-export full embedded-dsp suites for companding, dynamics, speech audio, and multi-rate resampling:
pub use embedded_dsp::audio::{
    GoertzelDetectorQ15, PeakEnvelopeFollower, PeakEnvelopeFollowerQ15, RmsEnvelopeFollower,
    RmsEnvelopeFollowerQ15, VadDetectorQ15, mel_filterbank_f32, mfcc_f32,
};
pub use embedded_dsp::companding::{
    a_law_compress_f32, a_law_expand_f32, alaw_to_linear, linear_to_alaw, linear_to_ulaw,
    mu_law_compress_f32, mu_law_expand_f32, ulaw_to_linear,
};
pub use embedded_dsp::dynamics::{DynamicsCompressor, NoiseGate};
pub use embedded_dsp::resampling::{
    CicDecimator as DspCicDecimator, CicInterpolator, resample_linear_f32, resample_linear_q15,
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

/// Single-frequency Goertzel algorithm detector for tone and DTMF decoding.
pub struct GoertzelDetector {
    coeff: f32,
    s_prev: f32,
    s_prev2: f32,
}

impl GoertzelDetector {
    /// Initialise a Goertzel detector for a target frequency and sample rate.
    pub fn new(target_freq: f32, sample_rate: f32) -> Self {
        let omega = 2.0 * core::f32::consts::PI * target_freq / sample_rate;
        let coeff = 2.0 * omega.cos();
        Self {
            coeff,
            s_prev: 0.0,
            s_prev2: 0.0,
        }
    }

    /// Reset state for a new window of samples.
    pub fn reset(&mut self) {
        self.s_prev = 0.0;
        self.s_prev2 = 0.0;
    }

    /// Process a single audio sample.
    pub fn update(&mut self, sample: f32) {
        let s = sample + self.coeff * self.s_prev - self.s_prev2;
        self.s_prev2 = self.s_prev;
        self.s_prev = s;
    }

    /// Compute the current magnitude at the target frequency.
    pub fn magnitude(&self) -> f32 {
        (self.s_prev * self.s_prev + self.s_prev2 * self.s_prev2
            - self.coeff * self.s_prev * self.s_prev2)
            .sqrt()
    }
}

/// Peak/RMS envelope follower with configurable attack and release smoothing.
pub struct EnvelopeFollower {
    attack_coeff: f32,
    release_coeff: f32,
    envelope: f32,
}

impl EnvelopeFollower {
    /// Create envelope follower given attack and release time constants in seconds and sample rate.
    pub fn new(attack_time_sec: f32, release_time_sec: f32, sample_rate: f32) -> Self {
        let attack_coeff = (-1.0 / (attack_time_sec * sample_rate)).exp();
        let release_coeff = (-1.0 / (release_time_sec * sample_rate)).exp();
        Self {
            attack_coeff,
            release_coeff,
            envelope: 0.0,
        }
    }

    /// Update envelope follower with incoming sample value.
    pub fn update(&mut self, sample: f32) -> f32 {
        let input_mag = sample.abs();
        if input_mag > self.envelope {
            self.envelope =
                self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * input_mag;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * input_mag;
        }
        self.envelope
    }

    /// Reset internal envelope state.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }
}

/// Fixed-point Q15 Biquad filter operating without hardware floating-point operations.
#[derive(Debug, Clone)]
pub struct BiquadAudioFilterQ15 {
    // Coefficients scaled in Q14 (1.14 fixed point format)
    b0: i16,
    b1: i16,
    b2: i16,
    a1: i16,
    a2: i16,
    x1: i16,
    x2: i16,
    y1: i16,
    y2: i16,
}

impl BiquadAudioFilterQ15 {
    pub const fn new(b0: i16, b1: i16, b2: i16, a1: i16, a2: i16) -> Self {
        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            x1: 0,
            x2: 0,
            y1: 0,
            y2: 0,
        }
    }

    pub fn reset(&mut self) {
        self.x1 = 0;
        self.x2 = 0;
        self.y1 = 0;
        self.y2 = 0;
    }

    /// Process a single 16-bit signed PCM sample (`i16`).
    pub fn process_sample_i16(&mut self, x: i16) -> i16 {
        let acc = (self.b0 as i32 * x as i32)
            + (self.b1 as i32 * self.x1 as i32)
            + (self.b2 as i32 * self.x2 as i32)
            - (self.a1 as i32 * self.y1 as i32)
            - (self.a2 as i32 * self.y2 as i32);

        let y = (acc >> 14).clamp(-32768, 32767) as i16;

        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;

        y
    }

    /// Process a single 8-bit signed PCM sample (`i8`).
    pub fn process_sample_i8(&mut self, x: i8) -> i8 {
        let x16 = (x as i16) << 8;
        let y16 = self.process_sample_i16(x16);
        (y16 >> 8) as i8
    }
}

/// Peak dynamic range compressor for embedded speakers and microphones.
#[derive(Debug, Clone)]
pub struct DynamicCompressor {
    threshold_linear: f32,
    ratio: f32,
    makeup_gain: f32,
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl DynamicCompressor {
    /// Create a new compressor.
    ///
    /// - `threshold_db`: level above which compression begins (e.g. -12.0 dB).
    /// - `ratio`: compression ratio (e.g. 4.0 for 4:1 compression; ratio >= 1.0).
    /// - `attack_sec`: attack time constant in seconds (e.g. 0.005 for 5 ms).
    /// - `release_sec`: release time constant in seconds (e.g. 0.100 for 100 ms).
    /// - `makeup_gain_db`: output makeup gain (e.g. 3.0 dB).
    /// - `sample_rate_hz`: audio sampling rate in Hz.
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_sec: f32,
        release_sec: f32,
        makeup_gain_db: f32,
        sample_rate_hz: f32,
    ) -> Self {
        let threshold_linear = 10.0f32.powf(threshold_db / 20.0);
        let makeup_gain = 10.0f32.powf(makeup_gain_db / 20.0);
        let attack_coeff = (-1.0 / (attack_sec * sample_rate_hz)).exp();
        let release_coeff = (-1.0 / (release_sec * sample_rate_hz)).exp();
        Self {
            threshold_linear,
            ratio: ratio.max(1.0),
            makeup_gain,
            envelope: 0.0,
            attack_coeff,
            release_coeff,
        }
    }

    /// Reset internal envelope state.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    /// Process a single floating-point sample in range `[-1.0, 1.0]`.
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let input_mag = input.abs();
        if input_mag > self.envelope {
            self.envelope =
                self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * input_mag;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * input_mag;
        }

        let gain = if self.envelope > self.threshold_linear && self.envelope > 1e-6 {
            let over_db = 20.0 * FloatMath::log10(self.envelope / self.threshold_linear);
            let compressed_over_db = over_db / self.ratio;
            let reduction_db = over_db - compressed_over_db;
            10.0f32.powf(-reduction_db / 20.0) * self.makeup_gain
        } else {
            self.makeup_gain
        };

        (input * gain).clamp(-1.0, 1.0)
    }

    /// Process an audio buffer of floating-point samples in place.
    pub fn process_buffer(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Process a signed 16-bit PCM sample (-32768..=32767).
    pub fn process_sample_i16(&mut self, sample: i16) -> i16 {
        let in_f32 = sample as f32 / 32768.0;
        let out_f32 = self.process_sample(in_f32);
        (out_f32 * 32767.0).clamp(-32768.0, 32767.0) as i16
    }

    /// Process a signed 8-bit PCM sample (-128..=127).
    pub fn process_sample_i8(&mut self, sample: i8) -> i8 {
        let in_f32 = sample as f32 / 128.0;
        let out_f32 = self.process_sample(in_f32);
        (out_f32 * 127.0).clamp(-128.0, 127.0) as i8
    }
}

/// 3rd-order Cascaded Integrator-Comb (CIC) filter for PDM digital microphone decimation.
///
/// Converts 1-bit high-frequency PDM bitstreams (from SPI, I2S, or timer capture)
/// into 16-bit linear PCM audio with zero heap allocation.
#[derive(Debug, Clone)]
pub struct PdmDecimator {
    // 3 Integrator stages (run at high PDM bit rate)
    i0: i64,
    i1: i64,
    i2: i64,
    // 3 Comb stages (run at decimated PCM rate)
    d0: i64,
    d1: i64,
    d2: i64,
    decimation_factor: u16,
    counter: u16,
    shift: u32,
}

impl PdmDecimator {
    /// Create a PDM decimator with specified decimation ratio `M` (e.g. 64 for 1.024 MHz PDM -> 16 kHz PCM).
    pub fn new(decimation_factor: u16) -> Self {
        let m = decimation_factor.max(8) as u64;
        let gain = m * m * m;
        let mut shift = 0;
        let mut g = gain;
        while g > 65536 {
            g >>= 1;
            shift += 1;
        }

        Self {
            i0: 0,
            i1: 0,
            i2: 0,
            d0: 0,
            d1: 0,
            d2: 0,
            decimation_factor: decimation_factor.max(8),
            counter: 0,
            shift,
        }
    }

    /// Reset internal filter integrators and delay registers to 0.
    pub fn reset(&mut self) {
        self.i0 = 0;
        self.i1 = 0;
        self.i2 = 0;
        self.d0 = 0;
        self.d1 = 0;
        self.d2 = 0;
        self.counter = 0;
    }

    /// Feed a single PDM 1-bit sample (`true` = logic high, `false` = logic low).
    /// Returns `Some(pcm_sample)` every `decimation_factor` bits.
    #[inline]
    pub fn feed_bit(&mut self, bit: bool) -> Option<i16> {
        let x: i64 = if bit { 1 } else { -1 };

        // 3 Integrator stages
        self.i0 = self.i0.wrapping_add(x);
        self.i1 = self.i1.wrapping_add(self.i0);
        self.i2 = self.i2.wrapping_add(self.i1);

        self.counter += 1;
        if self.counter >= self.decimation_factor {
            self.counter = 0;

            // 3 Comb stages at decimated rate
            let sample = self.i2;
            let c0 = sample.wrapping_sub(self.d0);
            self.d0 = sample;

            let c1 = c0.wrapping_sub(self.d1);
            self.d1 = c0;

            let c2 = c1.wrapping_sub(self.d2);
            self.d2 = c1;

            let scaled = (c2 >> self.shift).clamp(-32768, 32767) as i16;
            Some(scaled)
        } else {
            None
        }
    }

    /// Process a slice of packed PDM bytes (MSB first, as received from SPI/I2S DMA).
    ///
    /// Writes decimated 16-bit PCM samples into `pcm_out` and returns how many samples were written.
    pub fn process_pdm_bytes(&mut self, pdm_bytes: &[u8], pcm_out: &mut [i16]) -> usize {
        let mut out_idx = 0;
        for &byte in pdm_bytes {
            for bit_idx in (0..8).rev() {
                let bit = (byte & (1 << bit_idx)) != 0;
                if let Some(sample) = self.feed_bit(bit) {
                    if out_idx < pcm_out.len() {
                        pcm_out[out_idx] = sample;
                        out_idx += 1;
                    } else {
                        return out_idx;
                    }
                }
            }
        }
        out_idx
    }
}
