use crate::fixed::{hz_to_phase_inc, lerp_i8, phase_index, Phase};

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
