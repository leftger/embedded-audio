use embedded_audio::{
    markham, AdsrSpec, AudioEngine, FmMapper, Waveform,
};

#[test]
fn markham_profile_clamps_frequency() {
    assert_eq!(markham::clamp_frequency(100), 250);
    assert_eq!(markham::clamp_frequency(9000), 8000);
    assert_eq!(markham::clamp_frequency(1000), 1000);
}

#[test]
fn markham_fm_tick_tone() {
    let mut engine = AudioEngine::new_markham();
    engine.play_tone(440, 100, Waveform::Sine);
    let tick = engine.tick_fm();
    assert!(tick.active);
    assert!((430..=450).contains(&tick.frequency_hz));
}

#[test]
fn markham_fm_vco_silence() {
    let mapper = FmMapper::markham();
    let tick = mapper.map_pcm(0);
    assert!(!tick.active);
}
