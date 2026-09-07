//! Coverage-gap tests for less-exercised `embedded-audio` modules.

#![cfg(feature = "std")]

use embedded_audio::encode::adpcm::{encode_i16, encode_u8};

#[test]
fn adpcm_encoder_packs_expected_payload_length() {
    let pcm = [0i16, 100, -100, 200, -200];
    let out = encode_i16(&pcm);
    assert_eq!(out.len(), 4 + pcm.len() / 2 + pcm.len() % 2);

    let pcm8 = [128u8, 200, 100, 255, 64];
    let out8 = encode_u8(&pcm8);
    assert_eq!(out8.len(), 4 + pcm8.len() / 2 + pcm8.len() % 2);
}

#[cfg(feature = "fm")]
#[test]
fn fm_voice_start_next_and_stop() {
    use embedded_audio::synth::fm::FmVoice;

    let mut voice = FmVoice::new();
    assert!(!voice.is_active());
    assert_eq!(voice.next_sample(), None);

    voice.start(440, 200, 64, 48_000);
    assert!(voice.is_active());
    assert!(voice.next_sample().is_some());
    assert!(voice.next_sample().is_some());

    voice.stop();
    assert!(!voice.is_active());
    assert_eq!(voice.next_sample(), None);
}

#[test]
fn wavetable_voice_lifecycle_and_builtin_tables() {
    use embedded_audio::synth::wavetable::{
        PULSE_25_TABLE, SAW_TABLE, SINE_TABLE, SQUARE_TABLE, TRIANGLE_TABLE, WavetableVoice,
        generate_wavetable_fixed,
    };

    let mut voice = WavetableVoice::sine();
    assert!(!voice.is_active());
    assert_eq!(voice.next_sample(), None);

    voice.start(440, 48_000);
    assert!(voice.is_active());
    for _ in 0..16 {
        assert!(voice.next_sample().is_some());
    }
    voice.stop();
    assert!(!voice.is_active());
    assert_eq!(voice.next_sample(), None);

    // A short table should never emit samples.
    let mut short = WavetableVoice::new(&[128; 4]);
    short.start(440, 48_000);
    assert_eq!(short.next_sample(), None);

    assert_eq!(SINE_TABLE.len(), 256);
    assert_eq!(TRIANGLE_TABLE.len(), 256);
    assert_eq!(SAW_TABLE.len(), 256);
    assert_eq!(SQUARE_TABLE.len(), 256);
    assert_eq!(PULSE_25_TABLE.len(), 256);

    let generated = generate_wavetable_fixed(|_| -128);
    assert_eq!(generated[0], 0);
    let generated = generate_wavetable_fixed(|_| 127);
    assert_eq!(generated[0], 255);
}

#[test]
fn audio_error_and_effect_kind_coverage() {
    use embedded_audio::error::AudioError;
    use embedded_audio::tier::EffectKind;

    for (e, expected) in [
        (AudioError::InvalidBankMagic, "invalid bank magic"),
        (
            AudioError::UnsupportedBankVersion,
            "unsupported bank version",
        ),
        (AudioError::EffectNotFound, "effect not found"),
        (AudioError::InvalidEffectKind, "invalid effect kind"),
        (AudioError::TruncatedBank, "truncated bank blob"),
        (AudioError::NoBank, "no sound bank loaded"),
        (AudioError::BankFull, "bank builder capacity exceeded"),
        (AudioError::VoiceBusy, "no free voice"),
        (AudioError::PreviewIo, "preview file I/O failed"),
        (AudioError::InvalidPayload, "invalid effect payload"),
    ] {
        assert_eq!(e.as_str(), expected);
    }

    assert_eq!(EffectKind::from_u8(0), Some(EffectKind::Tone));
    assert_eq!(EffectKind::from_u8(1), Some(EffectKind::Wavetable));
    assert_eq!(EffectKind::from_u8(2), Some(EffectKind::Fm));
    assert_eq!(EffectKind::from_u8(3), Some(EffectKind::Pcm8));
    assert_eq!(EffectKind::from_u8(4), Some(EffectKind::Adpcm));
    assert_eq!(EffectKind::from_u8(5), Some(EffectKind::SigmaDeltaBits));
    assert_eq!(EffectKind::from_u8(6), None);
}

#[test]
fn sigma_delta_bitstream_samples_oneshot_and_loop() {
    use embedded_audio::stream::SigmaDeltaBitStream;
    use embedded_audio::tier::flags;

    let data = [0b1010_0000u8, 0b0101_0000u8];

    let mut one_shot = SigmaDeltaBitStream::new(&data, 0);
    assert_eq!(one_shot.next_sample(), Some(127));
    assert_eq!(one_shot.next_sample(), Some(-127));
    assert_eq!(one_shot.next_sample(), Some(127));
    assert_eq!(one_shot.next_sample(), Some(-127));
    for _ in 0..12 {
        assert!(one_shot.next_sample().is_some());
    }
    assert!(one_shot.is_done());
    assert_eq!(one_shot.next_sample(), None);

    let mut looping = SigmaDeltaBitStream::new(&data, flags::LOOP);
    assert_eq!(looping.next_sample(), Some(127));
    // Drain through end; a looping stream restarts rather than ending.
    for _ in 0..20 {
        assert!(looping.next_sample().is_some());
    }
    assert!(!looping.is_done());
}
