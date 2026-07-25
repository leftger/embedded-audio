use crate::fixed::{Phase, hz_to_phase_inc, lerp_i8, phase_index};
use crate::synth::wavetable::SINE_TABLE;

/// Tier A two-operator FM (modulator → carrier index).
#[derive(Debug, Clone, Copy)]
pub struct FmVoice {
    carrier_phase: Phase,
    carrier_inc: u32,
    mod_phase: Phase,
    mod_inc: u32,
    mod_depth_q8: u8,
    active: bool,
}

impl FmVoice {
    pub const fn new() -> Self {
        Self {
            carrier_phase: 0,
            carrier_inc: 0,
            mod_phase: 0,
            mod_inc: 0,
            mod_depth_q8: 64,
            active: false,
        }
    }

    /// `mod_ratio_cent` is mod/carrier ratio × 100 (e.g. 200 = 2.00).
    pub fn start(
        &mut self,
        carrier_hz: u32,
        mod_ratio_cent: u16,
        mod_depth_q8: u8,
        sample_rate_hz: u32,
    ) {
        self.carrier_phase = 0;
        self.mod_phase = 0;
        self.carrier_inc = hz_to_phase_inc(carrier_hz, sample_rate_hz);
        let mod_hz = (carrier_hz as u64 * mod_ratio_cent as u64) / 100;
        self.mod_inc = hz_to_phase_inc(mod_hz as u32, sample_rate_hz);
        self.mod_depth_q8 = mod_depth_q8;
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn next_sample(&mut self) -> Option<i8> {
        if !self.active {
            return None;
        }
        let mod_idx = phase_index(self.mod_phase);
        let mod_frac = (self.mod_phase >> 16) as u8;
        let ma = SINE_TABLE[mod_idx as usize] as i8;
        let mb = SINE_TABLE[mod_idx.wrapping_add(1) as usize] as i8;
        let mod_sample = lerp_i8(ma, mb, mod_frac);

        let mod_offset = ((mod_sample as i32 * self.mod_depth_q8 as i32) >> 8) as u8;
        let car_idx = phase_index(self.carrier_phase.wrapping_add((mod_offset as u32) << 24));
        let car_frac = (self.carrier_phase >> 16) as u8;
        let ca = SINE_TABLE[car_idx as usize] as i8;
        let cb = SINE_TABLE[car_idx.wrapping_add(1) as usize] as i8;
        let sample = lerp_i8(ca, cb, car_frac);

        self.carrier_phase = self.carrier_phase.wrapping_add(self.carrier_inc);
        self.mod_phase = self.mod_phase.wrapping_add(self.mod_inc);
        Some(sample)
    }
}
