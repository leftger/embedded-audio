#![cfg(feature = "dsp")]

use embedded_audio::dsp::{
    AudioLmsFilter, AudioMeter, AudioSpectrumAnalyzer, BiquadAudioFilter, WindowType,
};
use embedded_audio::prelude::*;
use embedded_audio::synth::Waveform;

#[test]
fn test_biquad_lowpass_filtering() {
    let mut filter = BiquadAudioFilter::lowpass(1000.0, 16000.0, 0.707);

    // DC signal (0 Hz) should pass through
    let mut dc = [1.0f32; 100];
    filter.process_buffer(&mut dc);
    // After settling, output should be close to 1.0
    assert!((dc[99] - 1.0).abs() < 0.05);

    // High frequency signal (7000 Hz at 16kHz sample rate) should be attenuated significantly
    filter.reset();
    let mut high_freq = [0.0f32; 100];
    for i in 0..100 {
        let t = i as f32 / 16000.0;
        high_freq[i] = (2.0 * core::f32::consts::PI * 7000.0 * t).sin();
    }
    filter.process_buffer(&mut high_freq);
    // Attenuated amplitude should be far below 1.0
    assert!(high_freq[99].abs() < 0.2);
}

#[test]
fn test_biquad_highpass_filtering() {
    let mut filter = BiquadAudioFilter::highpass(3000.0, 16000.0, 0.707);

    // Low frequency signal (100 Hz at 16kHz sample rate) should be attenuated
    let mut low_freq = [0.0f32; 100];
    for i in 0..100 {
        let t = i as f32 / 16000.0;
        low_freq[i] = (2.0 * core::f32::consts::PI * 100.0 * t).sin();
    }
    filter.process_buffer(&mut low_freq);
    assert!(low_freq[99].abs() < 0.15);

    // High frequency signal (6000 Hz at 16kHz sample rate) should pass through
    filter.reset();
    let mut high_freq = [0.0f32; 100];
    for i in 0..100 {
        let t = i as f32 / 16000.0;
        high_freq[i] = (2.0 * core::f32::consts::PI * 6000.0 * t).sin();
    }
    filter.process_buffer(&mut high_freq);
    assert!(high_freq[99].abs() > 0.7);
}

#[test]
fn test_biquad_bandpass_filtering() {
    let mut filter = BiquadAudioFilter::bandpass(1000.0, 16000.0, 1.0);

    // Center frequency signal (1000 Hz) should pass through with high magnitude
    let mut center_signal = [0.0f32; 128];
    for i in 0..128 {
        let t = i as f32 / 16000.0;
        center_signal[i] = (2.0 * core::f32::consts::PI * 1000.0 * t).sin();
    }
    filter.process_buffer(&mut center_signal);
    let center_peak = center_signal[64..]
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(center_peak > 0.3);

    // Off-center frequency signal (4000 Hz) should be significantly attenuated
    filter.reset();
    let mut off_center_signal = [0.0f32; 128];
    for i in 0..128 {
        let t = i as f32 / 16000.0;
        off_center_signal[i] = (2.0 * core::f32::consts::PI * 4000.0 * t).sin();
    }
    filter.process_buffer(&mut off_center_signal);
    let off_center_peak = off_center_signal[64..]
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(off_center_peak < 0.25);
}

#[test]
fn test_biquad_notch_filtering() {
    // 60 Hz hum notch filter at 1000 Hz sample rate
    let mut filter = BiquadAudioFilter::notch(60.0, 1000.0, 5.0);

    let mut hum_signal = [0.0f32; 200];
    for i in 0..200 {
        let t = i as f32 / 1000.0;
        hum_signal[i] = (2.0 * core::f32::consts::PI * 60.0 * t).sin();
    }
    filter.process_buffer(&mut hum_signal);
    assert!(hum_signal[199].abs() < 0.15);
}

#[test]
fn test_biquad_pcm8_filtering() {
    let mut filter = BiquadAudioFilter::lowpass(1000.0, 16000.0, 0.707);

    // Filter single PCM8 samples
    let sample_in: i8 = 100;
    let mut sample_out = 0i8;
    for _ in 0..50 {
        sample_out = filter.process_pcm8(sample_in);
    }
    // Settled output should approach input value
    assert!((sample_out - 100).abs() < 10);
}

#[test]
fn test_audio_engine_dsp_ticking_and_metering() {
    let mut engine = AudioEngine::from_sample_rate(16000, 255, DutyMode::Linear);
    engine.play_tone(440, 100, Waveform::Sine);

    let mut buf = [0.0f32; 256];
    let written = engine.fill_pcm_f32_buffer(&mut buf);
    assert_eq!(written, 256);

    let stats = AudioMeter::measure(&buf);
    assert!(stats.rms > 0.0);
    assert!(stats.peak > 0.0);
    assert!(stats.power > 0.0);
}

#[test]
fn test_audio_meter_known_sine_wave() {
    // Pure sine wave: peak = 1.0, theoretical RMS = 1 / sqrt(2) ≈ 0.7071
    let mut sine_buf = [0.0f32; 1000];
    for i in 0..1000 {
        let t = i as f32 / 1000.0;
        sine_buf[i] = (2.0 * core::f32::consts::PI * 10.0 * t).sin();
    }

    let stats = AudioMeter::measure(&sine_buf);
    assert!((stats.peak - 1.0).abs() < 0.01);
    assert!((stats.rms - 0.7071).abs() < 0.02);
    assert!(stats.mean.abs() < 0.01);
}

#[test]
fn test_spectrum_analyzer_pitch_detection() {
    let mut engine = AudioEngine::from_sample_rate(16000, 255, DutyMode::Linear);
    // Generate 1000 Hz sine wave tone
    engine.play_tone(1000, 100, Waveform::Sine);

    let mut buf = [0.0f32; 256];
    engine.fill_pcm_f32_buffer(&mut buf);

    let (peak_freq, peak_mag) =
        AudioSpectrumAnalyzer::find_peak_frequency(&buf, 16000.0, WindowType::Hanning);

    assert!(peak_mag > 0.0);
    // Peak frequency should be near 1000 Hz (within FFT bin width resolution: 16000 / 256 = 62.5 Hz)
    assert!((peak_freq - 1000.0).abs() <= 62.5);
}

#[test]
fn test_spectrum_analyzer_windowing_types() {
    let mut engine = AudioEngine::from_sample_rate(16000, 255, DutyMode::Linear);
    engine.play_tone(800, 100, Waveform::Sine);

    let mut buf = [0.0f32; 128];
    engine.fill_pcm_f32_buffer(&mut buf);

    let mut mag_hanning = [0.0f32; 64];
    let mut mag_hamming = [0.0f32; 64];
    let mut mag_blackman = [0.0f32; 64];

    AudioSpectrumAnalyzer::analyze_spectrum(&buf, WindowType::Hanning, &mut mag_hanning);
    AudioSpectrumAnalyzer::analyze_spectrum(&buf, WindowType::Hamming, &mut mag_hamming);
    AudioSpectrumAnalyzer::analyze_spectrum(&buf, WindowType::Blackman, &mut mag_blackman);

    // Bin for 800 Hz (800 / (16000 / 128) = 6.4 -> bin 6)
    assert!(mag_hanning[6] > 0.0);
    assert!(mag_hamming[6] > 0.0);
    assert!(mag_blackman[6] > 0.0);
}

#[test]
fn test_lms_adaptive_filter() {
    let mut coeffs = [0.0f32; 8];
    let mut state = [0.0f32; 8];
    let mut filter = AudioLmsFilter::new(8, &mut coeffs, &mut state, 0.01);

    let src = [0.5f32; 16];
    let ref_sig = [0.5f32; 16];
    let mut out = [0.0f32; 16];
    let mut err = [0.0f32; 16];

    filter.process(&src, &ref_sig, &mut out, &mut err);
    // Error should decrease over time
    assert!(err[15].abs() < err[0].abs());
}

#[test]
fn test_end_to_end_dsp_audio_pipeline() {
    let sample_rate = 16000;
    let mut engine = AudioEngine::from_sample_rate(sample_rate, 255, DutyMode::Linear);
    engine.play_tone(2000, 200, Waveform::Sine);

    // 1. Fill buffer from engine
    let mut pcm_buffer = [0.0f32; 256];
    engine.fill_pcm_f32_buffer(&mut pcm_buffer);

    // 2. Highpass filter to isolate high frequencies above 1000 Hz
    let mut filter = BiquadAudioFilter::highpass(1000.0, sample_rate as f32, 0.707);
    filter.process_buffer(&mut pcm_buffer);

    // 3. Measure filtered audio statistics
    let stats = AudioMeter::measure(&pcm_buffer);
    assert!(stats.rms > 0.0);

    // 4. Spectral analysis on filtered audio
    let (peak_freq, peak_mag) = AudioSpectrumAnalyzer::find_peak_frequency(
        &pcm_buffer,
        sample_rate as f32,
        WindowType::Hanning,
    );

    assert!(peak_mag > 0.0);
    assert!((peak_freq - 2000.0).abs() <= 62.5);
}
