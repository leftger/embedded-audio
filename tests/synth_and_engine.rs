use embedded_audio::envelope::{Adsr, AdsrSpec};
use embedded_audio::hal::{DutyBuffer, PwmDutySink, fill_buffer_into, tick_into};
use embedded_audio::prelude::*;
use embedded_audio::synth::wavetable::SINE_TABLE;
use embedded_audio::synth::{ToneParams, ToneVoice, Waveform, WavetableVoice};

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
