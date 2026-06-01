use embedded_audio::{
    preview::{pcm_i8_to_u8, render_effect_pcm, write_mono_u8},
    encode::wav::build_wav_u8,
    AdsrSpec, BankBuilder, EffectKind, SoundBank, flags, BANK_BUILD_CAP,
};

#[test]
fn render_tone_to_pcm() {
    let mut built = heapless::Vec::<u8, BANK_BUILD_CAP>::new();
    let mut b = BankBuilder::new(16_000);
    b.add_effect(1, EffectKind::Tone, flags::ONE_SHOT, 255, 440, 80, &[])
        .unwrap();
    b.finish(&mut built).unwrap();
    let bank = SoundBank::parse(&built).unwrap();
    let pcm = render_effect_pcm(bank, 1, 16_000, AdsrSpec::click()).unwrap();
    assert!(!pcm.is_empty());
    assert!(pcm.iter().any(|&s| s != 0));
}

#[test]
fn wav_roundtrip_header() {
    let samples = [128u8; 16];
    let wav = build_wav_u8(16_000, &samples);
    assert!(wav.starts_with(b"RIFF"));
    assert!(wav.get(8..12) == Some(b"WAVE"));
}

#[test]
fn pcm_i8_to_u8_centers() {
    let u = pcm_i8_to_u8(&[0, 127, -127]);
    assert_eq!(u[0], 128);
    assert_eq!(u[1], 255);
    assert_eq!(u[2], 1);
}

#[test]
fn write_wav_tmp() {
    let path = std::env::temp_dir().join("embedded_audio_test.wav");
    let path_str = path.to_str().unwrap();
    write_mono_u8(path_str, 16_000, &[128, 200, 80, 128]).unwrap();
    let data = std::fs::read(path_str).unwrap();
    assert!(data.len() > 44);
    let _ = std::fs::remove_file(path);
}
