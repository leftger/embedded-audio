use embedded_audio::envelope::{Adsr, AdsrSpec};
use embedded_audio::hal::{DmaDoubleBuffer, DutyBuffer, PwmDutySink, fill_buffer_into, tick_into};
use embedded_audio::prelude::*;
use embedded_audio::synth::{
    PULSE_25_TABLE, SAW_TABLE, SINE_TABLE, SQUARE_TABLE, TRIANGLE_TABLE, ToneParams, ToneVoice,
    Waveform, WavetableVoice, generate_wavetable_fixed,
};

#[test]
fn test_adsr_envelope_lifecycle() {
    let spec = AdsrSpec {
        attack_ms: 10,
        decay_ms: 10,
        sustain_q8: 128, // 50%
        release_ms: 10,
    };
    let mut env = Adsr::new(spec, 1000);

    // Trigger envelope
    env.trigger();
    assert!(env.is_active());

    // Tick through attack phase (10 ms at 1000 Hz = 10 samples)
    for _ in 0..10 {
        env.tick();
    }
    // At end of attack, level should reach peak (255)
    assert_eq!(env.level_q8(), 255);

    // Tick through decay phase to sustain level (128)
    for _ in 0..10 {
        env.tick();
    }
    assert!((env.level_q8() as i16 - 128).abs() <= 5);

    // Trigger release
    env.release();
    for _ in 0..10 {
        env.tick();
    }
    // After release, level should drop to 0
    assert_eq!(env.level_q8(), 0);
    assert!(!env.is_active());
}

#[test]
fn test_all_synth_waveforms_produce_samples() {
    let waveforms = [Waveform::Sine, Waveform::Triangle, Waveform::Square];

    for wf in waveforms {
        let mut voice = ToneVoice::new();
        voice.start(
            ToneParams {
                freq_hz: 440,
                duration_ms: 50,
                waveform: wf,
                adsr: AdsrSpec::click(),
            },
            16_000,
        );

        let mut samples = [0i8; 64];
        let mut active_count = 0;
        for s in samples.iter_mut() {
            if let Some(sample) = voice.next_sample() {
                *s = sample;
                active_count += 1;
            }
        }

        assert_eq!(active_count, 64, "Waveform {:?} should yield samples", wf);
        assert!(
            samples.iter().any(|&s| s != 0),
            "Waveform {:?} should produce non-zero samples",
            wf
        );
    }
}

#[test]
fn test_wavetable_voice_synthesis() {
    let mut voice = WavetableVoice::new(&SINE_TABLE);
    voice.start(440, 16_000);

    let mut samples_collected = 0;
    while let Some(_sample) = voice.next_sample() {
        samples_collected += 1;
        if samples_collected >= 100 {
            break;
        }
    }

    assert_eq!(samples_collected, 100);
}

struct MockPwmHardware {
    written_duties: heapless::Vec<u16, 64>,
}

impl PwmDutySink for MockPwmHardware {
    fn set_duty(&mut self, duty: u16) {
        let _ = self.written_duties.push(duty);
    }
}

#[test]
fn test_hal_tick_into_dma_sink() {
    let mut engine = AudioEngine::from_sample_rate(16000, 1000, DutyMode::Linear);
    engine.play_tone(440, 100, Waveform::Sine);

    let mut hw = MockPwmHardware {
        written_duties: heapless::Vec::new(),
    };

    tick_into(&mut engine, &mut hw);
    assert_eq!(hw.written_duties.len(), 1);

    let mut out_buf = [0u16; 32];
    let mut target = [0u16; 32];
    let mut duty_buf = DutyBuffer::new(&mut target);
    let written = fill_buffer_into(&mut engine, &mut out_buf, &mut duty_buf);
    assert_eq!(written, 32);
    assert_eq!(duty_buf.cursor, 1);
}

#[test]
fn test_audio_engine_stop_and_reset() {
    let mut engine = AudioEngine::from_sample_rate(16000, 255, DutyMode::Linear);
    engine.play_tone(440, 500, Waveform::Sine);
    assert!(engine.is_playing());

    engine.stop_all();
    // Tick through release phase
    for _ in 0..200 {
        engine.tick();
    }
    assert!(!engine.is_playing());
    assert_eq!(engine.tick_pcm(), 0);
}

#[test]
fn test_wavetable_presets_and_play_wavetable() {
    let mut engine = AudioEngine::from_sample_rate(16000, 1000, DutyMode::Linear);

    // Test play_wavetable with SAW_TABLE
    let voice_idx = engine
        .play_wavetable(&SAW_TABLE, 440, AdsrSpec::click())
        .expect("play wavetable saw");
    assert_eq!(voice_idx, 0);
    assert!(engine.is_playing());

    let mut pcm_buf = [0i8; 32];
    engine.fill_pcm_i8_buffer(&mut pcm_buf);
    assert!(pcm_buf.iter().any(|&s| s != 0));

    // Custom wavetable using generate_wavetable_fixed
    let custom_table = generate_wavetable_fixed(|phase_idx| {
        // Simple triangle wave mapping
        if phase_idx < 128 {
            (phase_idx as i16 - 64) as i8
        } else {
            (191 - phase_idx as i16) as i8
        }
    });
    assert_eq!(custom_table.len(), 256);

    engine.stop_all();
    engine
        .play_wavetable(&SQUARE_TABLE, 220, AdsrSpec::click())
        .unwrap();
    assert!(engine.is_playing());

    engine.stop_all();
    engine
        .play_wavetable(&PULSE_25_TABLE, 220, AdsrSpec::click())
        .unwrap();
    assert!(engine.is_playing());

    engine.stop_all();
    engine
        .play_wavetable(&custom_table, 880, AdsrSpec::click())
        .unwrap();
    assert!(engine.is_playing());
}

#[test]
fn test_peripheral_dma_buffer_filling() {
    let mut engine = AudioEngine::from_sample_rate(16000, 1000, DutyMode::Linear);
    engine
        .play_wavetable(&TRIANGLE_TABLE, 440, AdsrSpec::click())
        .unwrap();

    // 1. PCM i16
    let mut buf_i16 = [0i16; 16];
    engine.fill_pcm_i16_buffer(&mut buf_i16);
    assert!(buf_i16.iter().any(|&s| s != 0));

    // 2. PCM i32
    let mut buf_i32 = [0i32; 16];
    engine.fill_pcm_i32_buffer(&mut buf_i32);
    assert!(buf_i32.iter().any(|&s| s != 0));

    // 3. DAC u8
    let mut buf_u8 = [0u8; 16];
    engine.fill_dac_u8_buffer(&mut buf_u8);
    assert!(buf_u8.iter().any(|&s| s != 128));

    // 4. DAC u12 (0..4095)
    let mut buf_u12 = [0u16; 16];
    engine.fill_dac_u12_buffer(&mut buf_u12);
    assert!(buf_u12.iter().all(|&s| s <= 4095));
    assert!(buf_u12.iter().any(|&s| s != 2048));

    // 5. DAC u16 (0..65535)
    let mut buf_u16 = [0u16; 16];
    engine.fill_dac_u16_buffer(&mut buf_u16);
    assert!(buf_u16.iter().any(|&s| s != 32768));

    // 6. Stereo i16
    let mut buf_stereo_i16 = [0i16; 32];
    engine.fill_stereo_i16_buffer(&mut buf_stereo_i16);
    assert_eq!(buf_stereo_i16[0], buf_stereo_i16[1]);

    // 7. Stereo i32
    let mut buf_stereo_i32 = [0i32; 32];
    engine.fill_stereo_i32_buffer(&mut buf_stereo_i32);
    assert_eq!(buf_stereo_i32[0], buf_stereo_i32[1]);
}

#[test]
fn test_dma_double_buffer() {
    let mut dbuf = DmaDoubleBuffer::<u16, 64>::new();
    assert_eq!(DmaDoubleBuffer::<u16, 64>::HALF_SIZE, 64);
    assert_eq!(DmaDoubleBuffer::<u16, 64>::TOTAL_SIZE, 128);

    let mut engine = AudioEngine::from_sample_rate(16000, 1000, DutyMode::Linear);
    engine.play_tone(440, 100, Waveform::Sine);

    // Fill current half
    engine.fill_dac_u12_buffer(dbuf.current_half_mut());
    let active_sample = dbuf.current_half()[0];
    assert!(active_sample <= 4095);

    // Swap and fill next half
    let next_half = dbuf.swap_and_get_next();
    engine.fill_dac_u12_buffer(next_half);

    // Test fill_and_swap helper
    dbuf.fill_and_swap(|slice| {
        engine.fill_dac_u12_buffer(slice);
    });
}
