use embedded_audio::decode::{
    G711Format, G711Stream, g711_alaw_decode, g711_alaw_encode, g711_ulaw_decode, g711_ulaw_encode,
};
use embedded_audio::fx::{AntiPopRamp, DelayLine, RampState};
use embedded_audio::hal::{AudioProcessor, InPlaceAudioProcessor};
use embedded_audio::prelude::*;
use embedded_audio::synth::Waveform;

#[cfg(feature = "dsp")]
use embedded_audio::dsp::{DynamicCompressor, PdmDecimator};

#[test]
fn test_stereo_panning_gains() {
    let (left_center, right_center) = pan_to_gains_q8(128);
    assert_eq!(left_center, 255);
    assert_eq!(right_center, 255);

    let (left_full, right_full) = pan_to_gains_q8(0);
    assert_eq!(left_full, 255);
    assert_eq!(right_full, 0);

    let (left_zero, right_zero) = pan_to_gains_q8(255);
    assert_eq!(left_zero, 0);
    assert_eq!(right_zero, 255);

    let (left_mid, right_mid) = pan_to_gains_q8(64);
    assert_eq!(left_mid, 255);
    assert_eq!(right_mid, 127);
}

#[test]
fn test_soft_limiter() {
    // Within linear band (<= 96), output matches input exactly
    assert_eq!(soft_limit_i8(0), 0);
    assert_eq!(soft_limit_i8(50), 50);
    assert_eq!(soft_limit_i8(-50), -50);
    assert_eq!(soft_limit_i8(96), 96);
    assert_eq!(soft_limit_i8(-96), -96);

    // Beyond linear band, values smoothly compress towards ±127
    let lim_pos = soft_limit_i8(200);
    assert!(lim_pos > 96);

    let lim_huge = soft_limit_i8(10000);
    assert_eq!(lim_huge, 126);

    let lim_neg = soft_limit_i8(-200);
    assert!(lim_neg < -96);

    // 16-bit limiter
    assert_eq!(soft_limit_i16(10000), 10000);
    assert_eq!(soft_limit_i16(-10000), -10000);
    let lim16 = soft_limit_i16(50000);
    assert!(lim16 > 24576);
}

#[test]
fn test_engine_stereo_panning() {
    let mut engine = AudioEngine::<2>::with_voice_count(AudioConfig::default_duty());
    engine.play_tone(440, 100, Waveform::Square);

    // Hard pan voice 0 to the left
    engine.set_voice_pan(0, 0);
    assert_eq!(engine.voice_pan(0), Some(0));

    // Tick past initial ADSR attack ramp
    let mut left = 0;
    let mut right = 0;
    for _ in 0..10 {
        let (l, r) = engine.tick_stereo_pcm();
        left = l;
        right = r;
    }
    assert_ne!(left, 0);
    assert_eq!(right, 0);

    // Hard pan voice 0 to the right
    engine.set_voice_pan(0, 255);
    assert_eq!(engine.voice_pan(0), Some(255));

    let mut left_r = 0;
    let mut right_r = 0;
    for _ in 0..10 {
        let (l, r) = engine.tick_stereo_pcm();
        left_r = l;
        right_r = r;
    }
    assert_eq!(left_r, 0);
    assert_ne!(right_r, 0);

    // Interleaved stereo buffer fill
    let mut stereo_buf = [0i16; 64];
    let written = engine.fill_stereo_panned_i16_buffer(&mut stereo_buf);
    assert_eq!(written, 64);
    // Since voice 0 is hard-panned right:
    // Left channel (even indices) should be 0, Right channel (odd indices) should be non-zero
    assert_eq!(stereo_buf[0], 0);
    assert_ne!(stereo_buf[1], 0);
}

#[test]
fn test_delay_line_echo() {
    let mut delay = DelayLine::<100>::new(10);
    assert_eq!(delay.delay(), 10);

    delay.set_feedback_q8(200);
    delay.set_mix_q8(255, 0); // 100% wet

    // Feed single impulse
    let out0 = delay.process(100);
    assert_eq!(out0, 0); // initial output before delay time is 0

    // Advance 9 more samples
    for _ in 0..9 {
        assert_eq!(delay.process(0), 0);
    }

    // At sample 10, the delayed echo should arrive
    let echo1 = delay.process(0);
    assert!(echo1 > 0);

    delay.reset();
    assert_eq!(delay.process(0), 0);
}

#[test]
fn test_anti_pop_ramp() {
    let mut ramp = AntiPopRamp::new(10);
    assert!(ramp.is_muted());
    assert_eq!(ramp.tick_gain_q8(), 0);

    ramp.ramp_up();
    assert_eq!(ramp.state(), RampState::RampingUp);

    // Gain should increase monotonically to 255
    let mut last_gain = 0;
    for _ in 0..10 {
        let g = ramp.tick_gain_q8();
        assert!(g >= last_gain);
        last_gain = g;
    }
    assert_eq!(last_gain, 255);
    assert!(ramp.is_active());

    // Applying soft duty ramp
    let duty = ramp.apply_duty(1000);
    assert_eq!(duty, 1000);

    // Ramp down
    ramp.ramp_down();
    assert_eq!(ramp.state(), RampState::RampingDown);
    for _ in 0..10 {
        let _ = ramp.tick_gain_q8();
    }
    assert!(ramp.is_muted());
    assert_eq!(ramp.tick_gain_q8(), 0);
}

#[test]
fn test_g711_codecs() {
    // µ-law test vectors
    let pcm = 12345i16;
    let u_byte = g711_ulaw_encode(pcm);
    let u_decoded = g711_ulaw_decode(u_byte);
    // G.711 is lossy, but relative error should be small (< 5%)
    assert!((pcm - u_decoded).abs() < 500);

    // A-law test vectors
    let a_byte = g711_alaw_encode(pcm);
    let a_decoded = g711_alaw_decode(a_byte);
    assert!((pcm - a_decoded).abs() < 500);

    // Stream playback
    let raw_ulaw = [u_byte; 8];
    let mut stream = G711Stream::new(&raw_ulaw, G711Format::MuLaw, false);
    assert!(!stream.is_done());
    let s0 = stream.next_sample_i16().unwrap();
    assert_eq!(s0, u_decoded);

    let s_i8 = stream.next_sample_i8().unwrap();
    assert_eq!(s_i8, (u_decoded >> 8) as i8);
}

#[test]
fn test_audio_processor_traits() {
    struct TestGainProcessor;
    impl AudioProcessor<i16> for TestGainProcessor {
        fn process_frame(&mut self, input: &[i16], output: &mut [i16]) {
            for (i, o) in input.iter().zip(output.iter_mut()) {
                *o = i / 2;
            }
        }
    }

    impl InPlaceAudioProcessor<i16> for TestGainProcessor {
        fn process_in_place(&mut self, buffer: &mut [i16]) {
            for s in buffer.iter_mut() {
                *s /= 2;
            }
        }
    }

    let mut proc = TestGainProcessor;
    let input = [1000, -2000, 4000];
    let mut output = [0; 3];
    proc.process_frame(&input, &mut output);
    assert_eq!(output, [500, -1000, 2000]);

    proc.process_in_place(&mut output);
    assert_eq!(output, [250, -500, 1000]);
}

#[cfg(feature = "dsp")]
#[test]
fn test_dynamic_compressor() {
    // Threshold -12 dB, 4:1 ratio, 5ms attack, 50ms release, 0dB makeup
    let mut comp = DynamicCompressor::new(-12.0, 4.0, 0.005, 0.050, 0.0, 16000.0);

    // Small signal below threshold (-20 dB ≈ 0.1) passes almost unaffected
    let small_out = comp.process_sample(0.1);
    assert!((small_out - 0.1).abs() < 0.01);

    // Loud signal (1.0 = 0 dB) should be compressed significantly below 1.0
    comp.reset();
    let mut loud_out = 0.0;
    for _ in 0..200 {
        loud_out = comp.process_sample(1.0);
    }
    assert!(loud_out < 0.85);

    // i16 and i8 processing
    let sample_i16 = comp.process_sample_i16(1000);
    assert!(sample_i16 > 0);
}

#[cfg(feature = "dsp")]
#[test]
fn test_pdm_decimator() {
    let mut decimator = PdmDecimator::new(64);

    // Feed alternating bits (01010101 = silence / zero DC offset)
    let silence_pdm = [0b10101010u8; 16]; // 128 PDM bits -> should produce 2 PCM samples
    let mut pcm_out = [0i16; 4];
    let count = decimator.process_pdm_bytes(&silence_pdm, &mut pcm_out);
    assert_eq!(count, 2);

    // Feed positive bits (all 1s)
    let loud_pdm = [0xFFu8; 16];
    let count2 = decimator.process_pdm_bytes(&loud_pdm, &mut pcm_out);
    assert_eq!(count2, 2);
    // Positive PDM bitstream should produce positive PCM amplitude
    assert!(pcm_out[0] > 0);
}

#[cfg(feature = "dsp")]
#[test]
fn test_embedded_dsp_dynamics_and_companding() {
    use embedded_audio::dsp::{
        DynamicsCompressor, NoiseGate, a_law_compress_f32, a_law_expand_f32, linear_to_ulaw,
        mu_law_compress_f32, mu_law_expand_f32, ulaw_to_linear,
    };

    // 1. DynamicsCompressor from embedded-dsp
    let mut comp = DynamicsCompressor::new(-20.0, 4.0, 6.0, 0.005, 0.1, 0.0, 48000.0);
    let out = comp.process(0.5);
    assert!(out > 0.0 && out < 0.5);

    // 2. NoiseGate from embedded-dsp
    let mut gate = NoiseGate::new(-40.0, -30.0, 0.002, 0.05, 48000.0);
    // Quiet signal below -40 dB should be attenuated
    let quiet_out = gate.process(0.001);
    assert!(quiet_out.abs() < 0.001);

    // 3. Floating-point companding curves from embedded-dsp
    let original = 0.42f32;
    let mu_comp = mu_law_compress_f32(original);
    let mu_exp = mu_law_expand_f32(mu_comp);
    assert!((original - mu_exp).abs() < 1e-4);

    let a_comp = a_law_compress_f32(original);
    let a_exp = a_law_expand_f32(a_comp);
    assert!((original - a_exp).abs() < 1e-4);

    // 4. G.711 byte conversions from embedded-dsp
    let pcm = 12000i16;
    let u_byte = linear_to_ulaw(pcm);
    let u_recov = ulaw_to_linear(u_byte);
    assert!((pcm - u_recov).abs() < 500);
}
