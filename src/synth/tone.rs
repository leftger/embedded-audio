use crate::envelope::AdsrSpec;
use crate::fixed::{hz_to_phase_inc, sin_table, Phase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Square,
    Sine,
    Triangle,
}

#[derive(Debug, Clone, Copy)]
pub struct ToneParams {
    pub freq_hz: u32,
    pub duration_ms: u16,
    pub waveform: Waveform,
    pub adsr: AdsrSpec,
}

/// Tier A tone oscillator with optional auto-stop by duration.
#[derive(Debug, Clone, Copy)]
pub struct ToneVoice {
    phase: Phase,
    phase_inc: u32,
    waveform: Waveform,
    samples_left: u32,
    active: bool,
}

impl ToneVoice {
    pub const fn new() -> Self {
        Self {
            phase: 0,
            phase_inc: 0,
            waveform: Waveform::Sine,
            samples_left: 0,
            active: false,
        }
    }

    pub fn start(&mut self, params: ToneParams, sample_rate_hz: u32) {
        self.phase = 0;
        self.phase_inc = hz_to_phase_inc(params.freq_hz, sample_rate_hz);
        self.waveform = params.waveform;
        self.samples_left = if params.duration_ms == 0 {
            u32::MAX
        } else {
            (params.duration_ms as u32 * sample_rate_hz) / 1000
        };
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.active = false;
        self.samples_left = 0;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Carrier in Hz derived from the phase increment (for FM buzzer backends).
    pub const fn carrier_hz(&self, sample_rate_hz: u32) -> u32 {
        if sample_rate_hz == 0 || !self.active {
            0
        } else {
            (((self.phase_inc as u64) * (sample_rate_hz as u64)) >> 32) as u32
        }
    }

    pub fn next_sample(&mut self) -> Option<i8> {
        if !self.active {
            return None;
        }
        if self.samples_left == 0 {
            self.active = false;
            return None;
        }
        if self.samples_left != u32::MAX {
            self.samples_left -= 1;
        }
        let sample = match self.waveform {
            Waveform::Square => {
                if self.phase < 0x8000_0000 {
                    127
                } else {
                    -127
                }
            }
            Waveform::Sine => sin_table((self.phase >> 24) as u8),
            Waveform::Triangle => {
                let idx = (self.phase >> 24) as u8;
                let tri = if idx < 128 {
                    (idx as i32 * 2) - 127
                } else {
                    127 - ((idx - 128) as i32 * 2)
                };
                tri.clamp(-127, 127) as i8
            }
        };
        self.phase = self.phase.wrapping_add(self.phase_inc);
        Some(sample)
    }
}
