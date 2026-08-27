#![cfg(feature = "dsp")]

use embedded_audio::prelude::*;

fn assert_bounded_and_finite(samples: &[f32]) {
    for &s in samples {
        assert!(s.is_finite(), "drum output diverged to {s}");
        assert!(s.abs() < 100.0, "drum output blew up to {s}");
    }
}

#[test]
fn test_analog_bass_drum_trigger_produces_bounded_output() {
    let mut kick = AnalogBassDrum::new(16_000.0);
    kick.set_freq(55.0);
    kick.set_decay(0.4);
    kick.set_tone(0.2);
    kick.set_accent(0.8);

    let mut samples = [0.0f32; 8_000];
    samples[0] = kick.process(true);
    for s in samples.iter_mut().skip(1) {
        *s = kick.process(false);
    }

    assert_bounded_and_finite(&samples);
    assert!(
        samples.iter().any(|&s| s.abs() > 1e-3),
        "kick should produce audible output after being triggered"
    );
}

#[test]
fn test_analog_bass_drum_sustain_mode_is_periodic_and_bounded() {
    let mut kick = AnalogBassDrum::new(16_000.0);
    kick.set_sustain(true);
    kick.set_freq(80.0);

    let mut samples = [0.0f32; 4_000];
    samples[0] = kick.process(true);
    for s in samples.iter_mut().skip(1) {
        *s = kick.process(false);
    }
    assert_bounded_and_finite(&samples);
}

#[test]
fn test_analog_snare_drum_trigger_produces_bounded_output() {
    let mut snare = AnalogSnareDrum::new(16_000.0);
    snare.set_freq(180.0);
    snare.set_snappy(0.7);
    snare.set_tone(0.5);
    snare.set_decay(0.35);
    snare.set_accent(0.9);

    let mut samples = [0.0f32; 8_000];
    samples[0] = snare.process(true);
    for s in samples.iter_mut().skip(1) {
        *s = snare.process(false);
    }

    assert_bounded_and_finite(&samples);
    assert!(samples.iter().any(|&s| s.abs() > 1e-3));
}

#[test]
fn test_hihat_trigger_produces_bounded_output() {
    let mut hat = HiHat::new(16_000.0);
    hat.set_freq(3200.0);
    hat.set_tone(0.6);
    hat.set_decay(0.15);
    hat.set_noisiness(0.9);
    hat.set_accent(0.9);

    let mut samples = [0.0f32; 8_000];
    samples[0] = hat.process(true);
    for s in samples.iter_mut().skip(1) {
        *s = hat.process(false);
    }

    assert_bounded_and_finite(&samples);
    assert!(samples.iter().any(|&s| s.abs() > 1e-3));
}

/// Re-tunes and re-triggers each drum once per "hit" (every 400 samples = 25ms at 16kHz), holding
/// parameters steady between hits like a real sequencer would. Like DaisySP's originals, these
/// resonant models are only stable for coefficients held steady (or changed gradually) between
/// samples — retuning every single sample is an unrealistic, unsupported usage pattern that can
/// make the resonators diverge (see the module docs), so this test intentionally does not do that.
#[test]
fn test_drum_parameter_changes_between_hits_never_diverge() {
    let mut kick = AnalogBassDrum::new(16_000.0);
    let mut snare = AnalogSnareDrum::new(16_000.0);
    let mut hat = HiHat::new(16_000.0);

    for i in 0..8_000 {
        if i % 400 == 0 {
            let t = ((i / 400) % 10) as f32 / 10.0;
            kick.set_freq(30.0 + t * 200.0);
            kick.set_decay(t);
            kick.set_tone(t);
            snare.set_freq(100.0 + t * 400.0);
            snare.set_decay(t);
            snare.set_snappy(t);
            hat.set_freq(1000.0 + t * 6000.0);
            hat.set_decay(t);
            hat.set_noisiness(t);
        }

        let trigger = i % 400 == 0;
        assert!(kick.process(trigger).is_finite());
        assert!(snare.process(trigger).is_finite());
        assert!(hat.process(trigger).is_finite());
    }
}
