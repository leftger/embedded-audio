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

/// Convert decibel value (-48.0 dB .. 0.0 dB) to Q8 gain (0 ..= 255).
///
/// 0.0 dB maps to 255 (1.0x).
/// -6.0 dB maps to ~128 (0.5x).
/// -48.0 dB or lower maps to 0.
#[allow(clippy::approx_constant)]
pub fn db_to_q8(db: f32) -> u8 {
    if db >= 0.0 {
        return 255;
    }
    if db <= -48.0 {
        return 0;
    }
    // Fast fixed-point approximation for 10^(db / 20) in no_std without libm dependency
    // db is negative, so db / 20 is in range [-2.4, 0]
    // 10^x ≈ 1 + 2.3026*x + 2.651*x^2 + 2.03*x^3
    let x = db / 20.0;
    let lin = 1.0 + 2.3026 * x + 2.651 * x * x + 2.03 * x * x * x;
    (lin * 255.0).clamp(0.0, 255.0) as u8
}

/// Convert Q8 gain (0 ..= 255) to decibels (-inf .. 0.0 dB).
#[allow(clippy::approx_constant)]
pub fn q8_to_db(gain_q8: u8) -> f32 {
    if gain_q8 == 0 {
        return -96.0;
    }
    let lin = gain_q8 as f32 / 255.0;
    // Log base 10 approximation using log2: log10(x) = log2(x) * 0.30103
    // For lin in (0, 1], fast log2 approximation:
    let bits = lin.to_bits();
    let exp = ((bits >> 23) & 0xFF) as i32 - 127;
    let mantissa = f32::from_bits((bits & 0x007F_FFFF) | 0x3F80_0000);
    let log2_val = exp as f32 + (mantissa - 1.0) * (1.4427 - 0.4427 * (mantissa - 1.0));
    let log10_val = log2_val * 0.30103;
    (20.0 * log10_val).clamp(-96.0, 0.0)
}
