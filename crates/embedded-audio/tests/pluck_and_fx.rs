use embedded_audio::prelude::*;

#[test]
fn test_karplus_pluck_rings_then_auto_stops() {
    let mut string = KarplusPluck::<512>::new();
    assert!(!string.is_active());

    string.pluck(220, 100, 20, 16_000);
    assert!(string.is_active());

    let mut heard_nonzero = false;
    let mut samples = 0;
    while let Some(sample) = string.next_sample() {
        heard_nonzero |= sample != 0;
        samples += 1;
        assert!(samples < 1_000_000, "string never auto-stopped");
    }

    assert!(heard_nonzero, "plucked string produced only silence");
    assert!(!string.is_active());
    // 20ms @ 16kHz == 320 samples.
    assert_eq!(samples, 320);
}

#[test]
fn test_karplus_pluck_stop_is_immediate() {
    let mut string = KarplusPluck::<256>::new();
    string.pluck(440, 127, 0, 16_000);
    assert!(string.next_sample().is_some());
    string.stop();
    assert!(!string.is_active());
    assert!(string.next_sample().is_none());
}

#[test]
fn test_karplus_pluck_brightness_and_decay_stay_finite() {
    let mut string = KarplusPluck::<256>::new();
    string.set_brightness_q8(0);
    string.set_decay_q8(0);
    string.pluck(300, 127, 5, 16_000);
    for _ in 0..500 {
        string.next_sample();
    }
    // Reaching here without panicking/overflowing is the assertion; `i8` output is trivially bounded.
}

#[test]
fn test_overdrive_stays_in_range_and_zero_drive_mutes() {
    let mut od = Overdrive::new(0.0);
    for input in [-128i8, -1, 0, 1, 127] {
        assert_eq!(od.process(input), 0);
    }

    od.set_drive(0.4);
    let out = od.process(64);
    assert_ne!(out, 0, "moderate drive should produce audible output");
}

#[test]
fn test_wavefolder_identity_at_unity_gain_small_signal() {
    let folder = Wavefolder::new();
    // Below the fold threshold (|x| <= 1), the folder is a no-op.
    assert_eq!(folder.process(0), 0);
    let out = folder.process(50);
    assert!((out - 50).abs() <= 1, "small signal should pass through ~unchanged: {out}");
}

#[test]
fn test_wavefolder_folds_large_gain() {
    let mut folder = Wavefolder::new();
    folder.set_gain(4.0);
    // Just check it stays bounded (i8 return type guarantees this) and doesn't panic across a
    // sweep of inputs, including the extremes.
    for input in -128i8..=127 {
        let _ = folder.process(input);
    }
}

#[test]
fn test_tremolo_no_depth_is_passthrough() {
    let mut tremolo = Tremolo::new();
    tremolo.set_freq(5, 16_000);
    tremolo.set_depth_q8(0);
    for _ in 0..100 {
        // Q8 gain of 255 (not 256) is ~0.996x, so this is "no audible effect", not bit-exact.
        assert!((tremolo.process(100) - 100).abs() <= 1);
    }
}

#[test]
fn test_tremolo_full_depth_modulates_amplitude() {
    let mut tremolo = Tremolo::new();
    tremolo.set_freq(1000, 16_000);
    tremolo.set_depth_q8(255);

    let mut min_out = i8::MAX;
    let mut max_out = i8::MIN;
    for _ in 0..64 {
        let out = tremolo.process(100);
        min_out = min_out.min(out);
        max_out = max_out.max(out);
    }
    assert!(max_out > min_out, "full-depth tremolo should visibly vary amplitude");
}
