use crate::fixed::{Phase, hz_to_phase_inc, lerp_i8, phase_index};

/// Tier A wavetable oscillator (256-byte table, linear interpolation).
#[derive(Debug, Clone, Copy)]
pub struct WavetableVoice<'a> {
    table: &'a [u8],
    phase: Phase,
    phase_inc: u32,
    active: bool,
}

impl<'a> WavetableVoice<'a> {
    /// `table` must hold at least 256 samples (unsigned 8-bit, centered at 128).
    pub const fn new(table: &'a [u8]) -> Self {
        Self {
            table,
            phase: 0,
            phase_inc: 0,
            active: false,
        }
    }

    pub const fn sine() -> Self {
        Self::new(&SINE_TABLE)
    }

    pub const fn triangle() -> Self {
        Self::new(&TRIANGLE_TABLE)
    }

    pub const fn saw() -> Self {
        Self::new(&SAW_TABLE)
    }

    pub const fn square() -> Self {
        Self::new(&SQUARE_TABLE)
    }

    pub fn start(&mut self, freq_hz: u32, sample_rate_hz: u32) {
        self.phase = 0;
        self.phase_inc = hz_to_phase_inc(freq_hz, sample_rate_hz);
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn next_sample(&mut self) -> Option<i8> {
        if !self.active || self.table.len() < 256 {
            return None;
        }
        let idx = phase_index(self.phase);
        let frac = (self.phase >> 16) as u8;
        let a = self.table[idx as usize] as i8;
        let b = self.table[idx.wrapping_add(1) as usize] as i8;
        let sample = lerp_i8(a, b, frac);
        self.phase = self.phase.wrapping_add(self.phase_inc);
        Some(sample)
    }
}

/// Built-in 256-sample sine-ish wavetable for FM and defaults.
pub static SINE_TABLE: [u8; 256] = generate_sine_table();
/// Built-in 256-sample triangle wavetable.
pub static TRIANGLE_TABLE: [u8; 256] = generate_triangle_table();
/// Built-in 256-sample sawtooth wavetable.
pub static SAW_TABLE: [u8; 256] = generate_saw_table();
/// Built-in 256-sample 50% square wavetable.
pub static SQUARE_TABLE: [u8; 256] = generate_square_table();
/// Built-in 256-sample 25% pulse wavetable.
pub static PULSE_25_TABLE: [u8; 256] = generate_pulse_25_table();

const fn generate_sine_table() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let x = i as i32;
        let y = if x < 128 { x * 2 } else { (255 - x) * 2 };
        let quad = (y * (255 - y)) >> 8;
        let s = if x < 128 { quad } else { -quad };
        let u = s + 128;
        t[i] = if u < 0 {
            0
        } else if u > 255 {
            255
        } else {
            u as u8
        };
        i += 1;
    }
    t
}

const fn generate_triangle_table() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let val = if i < 64 {
            128 + i * 2
        } else if i < 192 {
            255 - (i - 64) * 2
        } else {
            (i - 192) * 2
        };
        t[i] = val as u8;
        i += 1;
    }
    t
}

const fn generate_saw_table() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = i as u8;
        i += 1;
    }
    t
}

const fn generate_square_table() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = if i < 128 { 255 } else { 0 };
        i += 1;
    }
    t
}

const fn generate_pulse_25_table() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = if i < 64 { 255 } else { 0 };
        i += 1;
    }
    t
}

/// Generate a 256-sample wavetable from a signed 8-bit sample mapping closure (-128..=127 -> 0..=255).
pub fn generate_wavetable_fixed<F: Fn(u8) -> i8>(f: F) -> [u8; 256] {
    let mut table = [0u8; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let sample = f(i as u8);
        *slot = (sample as i16 + 128).clamp(0, 255) as u8;
    }
    table
}
