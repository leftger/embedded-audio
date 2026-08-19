/// ADSR segment in milliseconds (0 = skip segment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdsrSpec {
    pub attack_ms: u16,
    pub decay_ms: u16,
    pub sustain_q8: u8,
    pub release_ms: u16,
}

impl AdsrSpec {
    pub const fn click() -> Self {
        Self {
            attack_ms: 2,
            decay_ms: 40,
            sustain_q8: 0,
            release_ms: 10,
        }
    }

    pub const fn pad() -> Self {
        Self {
            attack_ms: 5,
            decay_ms: 80,
            sustain_q8: 200,
            release_ms: 120,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdsrPhase {
    Attack,
    Decay,
    Sustain,
    Release,
    Idle,
}

/// Piecewise-linear ADSR in Q8 (0..=255).
#[derive(Debug, Clone, Copy)]
pub struct Adsr {
    spec: AdsrSpec,
    phase: AdsrPhase,
    level_q8: u8,
    samples_in_phase: u32,
    phase_length_samples: u32,
    sample_rate_hz: u32,
}

impl Adsr {
    pub const fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }

    pub const fn new(spec: AdsrSpec, sample_rate_hz: u32) -> Self {
        Self {
            spec,
            phase: AdsrPhase::Idle,
            level_q8: 0,
            samples_in_phase: 0,
            phase_length_samples: 0,
            sample_rate_hz,
        }
    }

    pub fn trigger(&mut self) {
        self.phase = AdsrPhase::Attack;
        self.level_q8 = 0;
        self.samples_in_phase = 0;
        self.phase_length_samples = ms_to_samples(self.spec.attack_ms, self.sample_rate_hz);
        if self.phase_length_samples == 0 {
            self.begin_decay();
        }
    }

    pub fn release(&mut self) {
        if matches!(self.phase, AdsrPhase::Idle) {
            return;
        }
        self.phase = AdsrPhase::Release;
        self.samples_in_phase = 0;
        self.phase_length_samples = ms_to_samples(self.spec.release_ms, self.sample_rate_hz);
        if self.phase_length_samples == 0 {
            self.phase = AdsrPhase::Idle;
            self.level_q8 = 0;
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.phase, AdsrPhase::Idle)
    }

    /// Current envelope level 0..=255.
    pub fn level_q8(&self) -> u8 {
        self.level_q8
    }

    pub fn tick(&mut self) {
        match self.phase {
            AdsrPhase::Idle => {}
            AdsrPhase::Attack => {
                self.advance_toward(255);
                if self.samples_in_phase >= self.phase_length_samples {
                    self.begin_decay();
                }
            }
            AdsrPhase::Decay => {
                self.advance_toward(self.spec.sustain_q8);
                if self.samples_in_phase >= self.phase_length_samples {
                    self.phase = AdsrPhase::Sustain;
                    self.level_q8 = self.spec.sustain_q8;
                    self.samples_in_phase = 0;
                }
            }
            AdsrPhase::Sustain => {}
            AdsrPhase::Release => {
                self.advance_toward(0);
                if self.samples_in_phase >= self.phase_length_samples {
                    self.phase = AdsrPhase::Idle;
                    self.level_q8 = 0;
                }
            }
        }
    }

    fn begin_decay(&mut self) {
        self.phase = AdsrPhase::Decay;
        self.samples_in_phase = 0;
        self.phase_length_samples = ms_to_samples(self.spec.decay_ms, self.sample_rate_hz);
        if self.phase_length_samples == 0 {
            self.phase = AdsrPhase::Sustain;
            self.level_q8 = self.spec.sustain_q8;
        }
    }

    fn advance_toward(&mut self, target: u8) {
        self.samples_in_phase += 1;
        if self.phase_length_samples == 0 {
            self.level_q8 = target;
            return;
        }
        let start = self.level_q8 as i32;
        let end = target as i32;
        let t = (self.samples_in_phase as i32).min(self.phase_length_samples as i32);
        let v = start + ((end - start) * t) / self.phase_length_samples as i32;
        self.level_q8 = v.clamp(0, 255) as u8;
    }
}

fn ms_to_samples(ms: u16, sample_rate_hz: u32) -> u32 {
    if ms == 0 || sample_rate_hz == 0 {
        return 0;
    }
    (ms as u32 * sample_rate_hz) / 1000
}
