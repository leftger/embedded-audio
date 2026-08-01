use embedded_audio::{
    AdsrSpec, AudioConfig, AudioEngine, BANK_BUILD_CAP, BankBuilder, DutyMode, EffectKind,
    PwmMapper, SoundBank, ToneVoice, Waveform, flags,
};

#[test]
fn tier_a_tone_produces_pwm_variation() {
    let mut engine = AudioEngine::new(AudioConfig::new(16_000, 1000, DutyMode::Linear));
    engine.play_tone(440, 50, Waveform::Sine);
    let mut duties = heapless::Vec::<u16, 128>::new();
    for _ in 0..128 {
        let _ = duties.push(engine.tick());
    }
    let min = duties.iter().copied().min().unwrap();
    let max = duties.iter().copied().max().unwrap();
    assert!(max > min, "tone should swing duty");
}

#[test]
fn tier_b_pcm_bank_playback() {
    let pcm: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(8));
    let mut built = heapless::Vec::<u8, BANK_BUILD_CAP>::new();
    let mut b = BankBuilder::new(16_000);
    b.add_effect(1, EffectKind::Pcm8, flags::ONE_SHOT, 255, 0, 0, &pcm)
        .unwrap();
    b.finish(&mut built).unwrap();
    let bank = SoundBank::parse(&built).unwrap();
    assert_eq!(bank.sample_rate_hz, 16_000);
    let mut engine = AudioEngine::new(AudioConfig::new(16_000, 1000, DutyMode::SigmaDelta));
    engine.set_bank(bank);
    engine.play(1, AdsrSpec::click()).unwrap();
    assert!(engine.is_playing());
    for _ in 0..64 {
        engine.tick();
    }
}

#[test]
fn tier_b_adpcm_stream_decodes_bytes() {
    let payload: [u8; 8] = [0xF4, 0x01, 10, 0, 0x97, 0x2C, 0, 0];
    let mut stream = embedded_audio::AdpcmStream::new(&payload, flags::ONE_SHOT).unwrap();
    assert!(stream.next_sample().is_some());
}

#[test]
fn tier_c_bitstream() {
    let bits = [0xAAu8, 0x55];
    let mut built = heapless::Vec::<u8, BANK_BUILD_CAP>::new();
    let mut b = BankBuilder::new(16_000);
    b.add_effect(
        2,
        EffectKind::SigmaDeltaBits,
        flags::ONE_SHOT,
        255,
        0,
        0,
        &bits,
    )
    .unwrap();
    b.finish(&mut built).unwrap();
    let bank = SoundBank::parse(&built).unwrap();
    let mut engine = AudioEngine::default();
    engine.set_bank(bank);
    engine.play(2, AdsrSpec::click()).unwrap();
    let d0 = engine.tick();
    let d1 = engine.tick();
    assert_ne!(d0, d1);
}

#[test]
fn bank_parse_rejects_bad_magic() {
    assert!(SoundBank::parse(&[0; 16]).is_err());
}

#[test]
fn bank_v1_rejected() {
    let mut blob = [0u8; 32];
    blob[0..4].copy_from_slice(b"EAFX");
    blob[4] = 1; // old version
    assert!(matches!(
        SoundBank::parse(&blob),
        Err(embedded_audio::AudioError::UnsupportedBankVersion)
    ));
}

#[test]
fn sigma_delta_shapes_output() {
    let mut mapper = PwmMapper::new(DutyMode::SigmaDelta);
    let d1 = mapper.map(100, 1000);
    let d2 = mapper.map(-100, 1000);
    assert_ne!(d1, d2);
}

#[test]
fn crossfade_auto_completes() {
    let pcm_a: [u8; 16] = [128; 16];
    let pcm_b: [u8; 16] = [200; 16];
    let mut built = heapless::Vec::<u8, BANK_BUILD_CAP>::new();
    let mut b = BankBuilder::new(16_000);
    b.add_effect(1, EffectKind::Pcm8, flags::ONE_SHOT, 255, 0, 0, &pcm_a)
        .unwrap();
    b.add_effect(2, EffectKind::Pcm8, flags::ONE_SHOT, 255, 0, 0, &pcm_b)
        .unwrap();
    b.finish(&mut built).unwrap();
    let bank = SoundBank::parse(&built).unwrap();
    let mut engine = AudioEngine::default();
    engine.set_bank(bank);
    engine.play(1, AdsrSpec::pad()).unwrap();
    engine.crossfade_to(2, 10, AdsrSpec::pad()).unwrap();
    for _ in 0..500 {
        engine.tick();
    }
    assert!(!engine.is_playing() || engine.tick() > 0);
}

#[test]
fn play_respects_priority() {
    let pcm: [u8; 64] = [128; 64];
    let mut built = heapless::Vec::<u8, BANK_BUILD_CAP>::new();
    let mut b = BankBuilder::new(16_000);
    b.add_effect(1, EffectKind::Pcm8, flags::LOOP, 255, 0, 0, &pcm)
        .unwrap();
    b.finish(&mut built).unwrap();
    let bank = SoundBank::parse(&built).unwrap();
    let mut engine = AudioEngine::default();
    engine.set_bank(bank);
    engine.play_with_priority(1, AdsrSpec::pad(), 200).unwrap(); // occupies voice 0
    engine.play_with_priority(1, AdsrSpec::pad(), 200).unwrap(); // occupies voice 1
    assert!(engine.play_with_priority(1, AdsrSpec::pad(), 100).is_err()); // all voices busy with higher priority
    engine.stop_all();
}

#[test]
fn tone_voice_square_alternates() {
    let mut v = ToneVoice::new();
    v.start(
        embedded_audio::ToneParams {
            freq_hz: 1000,
            duration_ms: 10,
            waveform: Waveform::Square,
            adsr: AdsrSpec::click(),
        },
        16_000,
    );
    let mut signs = None;
    for _ in 0..32 {
        let s = v.next_sample().unwrap();
        if let Some(prev) = signs {
            if prev != s.signum() {
                return;
            }
        }
        signs = Some(s.signum());
    }
    panic!("square tone should change sign within 32 samples");
}
