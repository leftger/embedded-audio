/// 32-bit phase accumulator (upper bits index wavetables).
pub type Phase = u32;

/// Convert Hz to phase increment per sample: `(freq << 32) / sample_rate`.
#[inline]
pub const fn hz_to_phase_inc(freq_hz: u32, sample_rate_hz: u32) -> u32 {
    if sample_rate_hz == 0 {
        0
    } else {
        (((freq_hz as u64) << 32) / (sample_rate_hz as u64)) as u32
    }
}

/// Upper 8 bits of phase, 0..=255.
#[inline]
pub const fn phase_index(phase: Phase) -> u8 {
    (phase >> 24) as u8
}

/// Parabolic sine approximation on 0..=255 index → -127..=127.
#[inline]
pub fn sin_table(index: u8) -> i8 {
    let x = index as i32;
    let y = if x < 128 { x * 2 } else { (255 - x) * 2 };
    let quad = (y * (255 - y)) >> 8;
    let s = if x < 128 { quad } else { -quad };
    s.clamp(-127, 127) as i8
}

/// Linear interpolation between two i8 table entries; `frac` is 0..=255.
#[inline]
pub fn lerp_i8(a: i8, b: i8, frac: u8) -> i8 {
    let ai = a as i32;
    let bi = b as i32;
    let v = ai + ((bi - ai) * frac as i32) / 256;
    v.clamp(-128, 127) as i8
}

/// Apply Q8 gain (0..=255) to sample.
#[inline]
pub fn apply_gain_q8(sample: i8, gain_q8: u8) -> i8 {
    ((sample as i32 * gain_q8 as i32) >> 8).clamp(-128, 127) as i8
}

/// Mix two samples with crossfade weight `t_q8` (0 = a only, 255 = b only).
#[inline]
pub fn mix_crossfade(a: i8, b: i8, t_q8: u8) -> i8 {
    let ai = a as i32;
    let bi = b as i32;
    let t = t_q8 as i32;
    let v = ai + ((bi - ai) * t) / 256;
    v.clamp(-128, 127) as i8
}

/// Soft clamp to 8-bit audio range.
#[inline]
pub fn clamp_sample(v: i32) -> i8 {
    v.clamp(-128, 127) as i8
}
