use embedded_audio::decode::Pcm8Stream;
use embedded_audio::prelude::*;
use embedded_audio::synth::Waveform;

#[cfg(feature = "dsp")]
use embedded_audio::dsp::BiquadAudioFilterQ15;

#[test]
fn test_const_generic_4_voice_polyphony() {
    let mut engine = AudioEngine::<4>::with_voice_count(AudioConfig::default_duty());
    assert_eq!(engine.voice_count(), 4);
    assert_eq!(engine.active_voice_count(), 0);

    engine.play_tone(440, 100, Waveform::Sine);
    assert!(engine.is_playing());
}

#[test]
fn test_voice_stealing_policy() {
    let mut engine = AudioEngine::<2>::with_voice_count(AudioConfig::default_duty());
    engine.set_stealing_policy(VoiceStealingPolicy::FreeChannelOnly);
    assert_eq!(
        engine.stealing_policy(),
        VoiceStealingPolicy::FreeChannelOnly
    );
}

#[test]
fn test_fractional_speed_resampling() {
    let raw: [u8; 4] = [100, 120, 140, 160];
    let mut stream = Pcm8Stream::with_speed(&raw, 0, 32768);
    assert_eq!(stream.speed_q16(), 32768);
    let s0 = stream.next_sample().unwrap();
    let s1 = stream.next_sample().unwrap();
    assert!(s0 != s1 || s0 == raw[0] as i8);
}

#[test]
fn test_decibel_conversions() {
    assert_eq!(db_to_q8(0.0), 255);
    assert_eq!(db_to_q8(-48.0), 0);
    let half_gain = db_to_q8(-6.0);
    assert!((half_gain as i16 - 128).abs() <= 5);

    let db_full = q8_to_db(255);
    assert!(db_full.abs() <= 0.5);
    let db_zero = q8_to_db(0);
    assert_eq!(db_zero, -96.0);
}

#[test]
fn test_sigma_delta_2nd_order() {
    let mut mapper = PwmMapper::new(DutyMode::SigmaDelta2ndOrder);
    let d0 = mapper.map(0, 1000);
    let d1 = mapper.map(100, 1000);
    assert!(d0 > 0 && d0 < 1000);
    assert!(d1 > 0 && d1 < 1000);
}

#[cfg(feature = "dsp")]
#[test]
fn test_biquad_filter_q15() {
    let mut filter = BiquadAudioFilterQ15::new(16384, 0, 0, 0, 0); // unity gain Q14
    let out = filter.process_sample_i16(1000);
    assert_eq!(out, 1000);

    filter.reset();
    let out8 = filter.process_sample_i8(50);
    assert_eq!(out8, 50);
}

#[test]
fn test_closure_pwm_duty_sink() {
    let mut engine = AudioEngine::default();
    let mut captured_duty = 0u16;

    tick_into(&mut engine, &mut |duty| {
        captured_duty = duty;
    });

    assert!(captured_duty > 0);
}

#[test]
fn test_fill_dma_half_buffers() {
    let mut engine = AudioEngine::default();
    let mut half_buf = [0u16; 16];
    let written = fill_dma_half_buffers(&mut engine, &mut half_buf);
    assert_eq!(written, 16);
    assert!(half_buf.iter().any(|&d| d > 0));
}
