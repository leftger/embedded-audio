use crate::envelope::{Adsr, AdsrSpec};
use crate::fixed::apply_gain_q8;
use crate::source::VoiceSource;

/// One of two mixer voices.
#[derive(Debug, Clone, Copy)]
pub struct Voice<'a> {
    pub source: VoiceSource<'a>,
    adsr: Adsr,
    gain_q8: u8,
    pub priority: u8,
    sample_rate_hz: u32,
}

impl<'a> Voice<'a> {
    pub const fn silent(sample_rate_hz: u32) -> Self {
        Self {
            source: VoiceSource::idle(),
            adsr: Adsr::new(AdsrSpec::click(), sample_rate_hz),
            gain_q8: 255,
            priority: 0,
            sample_rate_hz,
        }
    }

    pub fn set_gain_q8(&mut self, gain_q8: u8) {
        self.gain_q8 = gain_q8;
    }

    pub fn gain_q8(self) -> u8 {
        self.gain_q8
    }

    pub fn trigger_adsr(&mut self, spec: AdsrSpec) {
        self.adsr = Adsr::new(spec, self.sample_rate_hz);
        self.adsr.trigger();
    }

    pub fn release(&mut self) {
        self.adsr.release();
    }

    pub fn stop_immediate(&mut self) {
        self.source.stop();
        self.adsr.release();
    }

    pub fn is_audible(&self) -> bool {
        self.source.is_active() || self.adsr.is_active()
    }

    pub fn tick_envelope(&mut self) {
        self.adsr.tick();
        if !self.source.is_active() && !self.adsr.is_active() {
            self.source.stop();
        }
    }

    /// Sample after envelope and per-voice gain.
    pub fn next_sample(&mut self) -> Option<i8> {
        let raw = self.source.next_raw_sample()?;
        let env = self.adsr.level_q8();
        let s = apply_gain_q8(raw, env);
        Some(apply_gain_q8(s, self.gain_q8))
    }
}
