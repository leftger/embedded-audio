use embedded_audio::{
    hal::DutyBuffer, AdsrSpec, AudioConfig, AudioEngine, BankBuilder, EffectKind, SoundBank,
    Waveform, flags, BANK_BUILD_CAP,
};

#[test]
fn fill_duty_buffer_matches_tick() {
    let cfg = AudioConfig::new(16_000, 1000, embedded_audio::DutyMode::Linear);
    let mut a = AudioEngine::new(cfg);
    a.play_tone(660, 30, Waveform::Sine);
    let single = a.tick();

    let mut b = AudioEngine::new(cfg);
    b.play_tone(660, 30, Waveform::Sine);
    let mut buf = [0u16; 1];
    b.fill_duty_buffer(&mut buf);
    assert_eq!(buf[0], single);
}

#[test]
fn duty_buffer_sink() {
    use embedded_audio::hal::tick_into;

    let mut engine = AudioEngine::default();
    engine.play_tone(440, 20, Waveform::Sine);
    let mut storage = [0u16; 32];
    let mut sink = DutyBuffer::new(&mut storage);
    for _ in 0..32 {
        tick_into(&mut engine, &mut sink);
    }
    assert!(storage.iter().any(|&d| d > 0));
}

#[test]
fn wavetable_from_bank_payload() {
    let table: [u8; 256] = core::array::from_fn(|i| (i as u8).wrapping_add(128));
    let mut built = heapless::Vec::<u8, BANK_BUILD_CAP>::new();
    let mut b = BankBuilder::new(16_000);
    b.add_effect(
        10,
        EffectKind::Wavetable,
        flags::ONE_SHOT,
        255,
        500,
        0,
        &table,
    )
    .unwrap();
    b.finish(&mut built).unwrap();
    let bank = SoundBank::parse(&built).unwrap();
    let mut engine = AudioEngine::default();
    engine.set_bank(bank);
    engine.play(10, AdsrSpec::click()).unwrap();
    let mut seen = false;
    for _ in 0..64 {
        if engine.tick() != engine.config().pwm_period / 2 {
            seen = true;
            break;
        }
    }
    assert!(seen);
}

#[test]
fn multi_effect_bank_lists_all_ids() {
    let pcm: [u8; 8] = [128, 140, 120, 128, 128, 128, 128, 128];
    let mut built = heapless::Vec::<u8, BANK_BUILD_CAP>::new();
    let mut b = BankBuilder::new(16_000);
    b.add_effect(1, EffectKind::Pcm8, flags::ONE_SHOT, 255, 0, 0, &pcm)
        .unwrap();
    b.add_effect(2, EffectKind::Tone, flags::ONE_SHOT, 255, 880, 50, &[])
        .unwrap();
    b.finish(&mut built).unwrap();
    let bank = SoundBank::parse(&built).unwrap();
    assert_eq!(bank.effect_count(), 2);
    assert!(bank.find_by_id(1).is_ok());
    assert!(bank.find_by_id(2).is_ok());
    assert!(bank.find_by_id(99).is_err());
}
