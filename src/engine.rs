use crate::bank::{EffectEntry, SoundBank};
use crate::config::{AudioConfig, crossfade_step_q8};
use crate::envelope::AdsrSpec;
use crate::error::AudioError;
use crate::fixed::{apply_gain_q8, mix_crossfade};
use crate::output::{DutyMode, PwmMapper, limit_bus};
use crate::voice::Voice;

#[cfg(feature = "fm")]
use crate::output::{FmMapper, FmTick};

const VOICES: usize = 2;

/// Two-voice mixer with bank playback and duty-modulated PWM output.
pub struct AudioEngine<'a> {
    bank: Option<SoundBank<'a>>,
    config: AudioConfig,
    voices: [Voice<'a>; VOICES],
    mapper: PwmMapper,
    crossfade_t_q8: u8,
    crossfade_step_q8: u8,
    crossfade_active: bool,
    #[cfg(feature = "fm")]
    fm_mapper: FmMapper,
}

impl<'a> AudioEngine<'a> {
    pub fn new(config: AudioConfig) -> Self {
        let rate = config.sample_rate_hz;
        Self {
            bank: None,
            config,
            voices: [Voice::silent(rate); VOICES],
            mapper: PwmMapper::new(config.duty_mode),
            crossfade_t_q8: 0,
            crossfade_step_q8: 0,
            crossfade_active: false,
            #[cfg(feature = "fm")]
            fm_mapper: FmMapper::markham(),
        }
    }

    pub fn from_sample_rate(sample_rate_hz: u32, pwm_period: u16, duty_mode: DutyMode) -> Self {
        Self::new(AudioConfig::new(sample_rate_hz, pwm_period, duty_mode))
    }

    pub fn config(&self) -> AudioConfig {
        self.config
    }

    pub fn set_bank(&mut self, bank: SoundBank<'a>) {
        let rate = bank.sample_rate_hz;
        self.config.sample_rate_hz = rate;
        self.voices = [Voice::silent(rate); VOICES];
        self.bank = Some(bank);
    }

    pub fn set_master_gain_q8(&mut self, gain: u8) {
        self.config.master_gain_q8 = gain;
    }

    pub fn stop_all(&mut self) {
        for v in &mut self.voices {
            v.stop_immediate();
        }
        self.crossfade_active = false;
        self.crossfade_step_q8 = 0;
        self.mapper.reset();
    }

    pub fn is_playing(&self) -> bool {
        self.voices.iter().any(|v| v.is_audible())
    }

    /// Play effect on voice 0 (replaces current voice 0).
    pub fn play(&mut self, effect_id: u16, adsr: AdsrSpec) -> Result<(), AudioError> {
        self.play_with_priority(effect_id, adsr, 128)
    }

    /// Play only if `priority` exceeds the active voice 0 priority.
    pub fn play_with_priority(
        &mut self,
        effect_id: u16,
        adsr: AdsrSpec,
        priority: u8,
    ) -> Result<(), AudioError> {
        if self.voices[0].is_audible() && self.voices[0].priority > priority {
            return Err(AudioError::VoiceBusy);
        }
        let bank = self.bank.ok_or(AudioError::NoBank)?;
        let entry = bank.find_by_id(effect_id)?;
        self.start_on_voice(0, &bank, entry, adsr, priority)
    }

    /// Crossfade voice 0 → `effect_id` on voice 1 over `duration_ms`.
    pub fn crossfade_to(
        &mut self,
        effect_id: u16,
        duration_ms: u16,
        adsr: AdsrSpec,
    ) -> Result<(), AudioError> {
        let bank = self.bank.ok_or(AudioError::NoBank)?;
        let entry = bank.find_by_id(effect_id)?;
        self.voices[0].release();
        self.start_on_voice(1, &bank, entry, adsr, 128)?;
        self.crossfade_active = true;
        self.crossfade_t_q8 = 0;
        self.crossfade_step_q8 = crossfade_step_q8(duration_ms, self.config.sample_rate_hz);
        Ok(())
    }

    fn advance_crossfade(&mut self) {
        if !self.crossfade_active {
            return;
        }
        let t = self.crossfade_t_q8.saturating_add(self.crossfade_step_q8);
        self.crossfade_t_q8 = t;
        if t == 255 {
            self.voices[0].stop_immediate();
            self.voices[0].source = self.voices[1].source;
            self.voices[0].set_gain_q8(self.voices[1].gain_q8());
            self.voices[0].priority = self.voices[1].priority;
            self.voices[1].stop_immediate();
            self.crossfade_active = false;
            self.crossfade_t_q8 = 0;
        }
    }

    fn start_on_voice(
        &mut self,
        idx: usize,
        bank: &SoundBank<'a>,
        entry: EffectEntry,
        adsr: AdsrSpec,
        priority: u8,
    ) -> Result<(), AudioError> {
        let payload = bank.payload(&entry)?;
        let rate = bank.sample_rate_hz;
        let mut voice = Voice::silent(rate);
        voice.set_gain_q8(entry.default_gain_q8);
        voice.priority = priority;
        if !voice.source.start_from_entry(
            entry.kind,
            entry.flags,
            entry.param0,
            entry.param1,
            payload,
            rate,
        ) {
            return Err(AudioError::InvalidEffectKind);
        }
        voice.trigger_adsr(adsr);
        self.voices[idx] = voice;
        Ok(())
    }

    fn tick_mixed_pcm(&mut self) -> i8 {
        self.advance_crossfade();
        for v in &mut self.voices {
            v.tick_envelope();
        }

        let a = self.voices[0].next_sample();
        let b = self.voices[1].next_sample();

        let mixed = match (a, b) {
            (None, None) => 0,
            (Some(s), None) => s as i32,
            (None, Some(s)) => s as i32,
            (Some(sa), Some(sb)) => {
                if self.crossfade_active {
                    mix_crossfade(sa, sb, self.crossfade_t_q8) as i32
                } else {
                    (sa as i32 + sb as i32) / 2
                }
            }
        };

        apply_gain_q8(limit_bus(mixed), self.config.master_gain_q8)
    }

    /// Mixed PCM sample after envelopes (before PWM mapping). Useful for WAV preview.
    pub fn tick_pcm(&mut self) -> i8 {
        self.tick_mixed_pcm()
    }

    #[cfg(feature = "dsp")]
    /// Mixed PCM sample tick normalized to floating-point range `[-1.0, 1.0]`.
    pub fn tick_pcm_f32(&mut self) -> f32 {
        self.tick_mixed_pcm() as f32 / 128.0
    }

    #[cfg(feature = "dsp")]
    /// Fill a buffer with consecutive normalized floating-point PCM samples.
    pub fn fill_pcm_f32_buffer(&mut self, out: &mut [f32]) -> usize {
        for sample in out.iter_mut() {
            *sample = self.tick_pcm_f32();
        }
        out.len()
    }

    /// One audio sample tick → PWM duty compare value.
    pub fn tick(&mut self) -> u16 {
        let pcm = self.tick_mixed_pcm();
        self.mapper.map(pcm, self.config.pwm_period)
    }

    /// Fill a DMA buffer with consecutive duty values (one per sample tick).
    ///
    /// Returns how many slots were written (`out.len()`).
    pub fn fill_duty_buffer(&mut self, out: &mut [u16]) -> usize {
        let period = self.config.pwm_period;
        for duty in out.iter_mut() {
            let pcm = self.tick_mixed_pcm();
            *duty = self.mapper.map(pcm, period);
        }
        out.len()
    }

    /// Alias for [`Self::tick`].
    #[inline]
    pub fn tick_pwm(&mut self) -> u16 {
        self.tick()
    }

    /// Tier A tone without a bank.
    pub fn play_tone(&mut self, freq_hz: u32, duration_ms: u16, waveform: crate::synth::Waveform) {
        let rate = self.config.sample_rate_hz;
        self.voices[0] = Voice::silent(rate);
        self.voices[0]
            .source
            .start_tone(freq_hz, duration_ms, waveform, rate);
        self.voices[0].trigger_adsr(AdsrSpec::click());
    }

    #[cfg(feature = "fm")]
    pub fn set_fm_mapper(&mut self, mapper: FmMapper) {
        self.fm_mapper = mapper;
    }

    #[cfg(feature = "fm")]
    /// FM buzzer backend (optional; not used for duty-modulation products).
    pub fn tick_fm(&mut self) -> FmTick {
        let pcm = self.tick_mixed_pcm();
        let rate = self.config.sample_rate_hz;

        if !self.crossfade_active {
            if let Some(hz) = self.voices[0].source.carrier_hz(rate) {
                return FmMapper::from_carrier(hz, self.voices[0].source.is_active());
            }
        }

        self.fm_mapper.map_pcm(pcm)
    }

    #[cfg(feature = "fm")]
    pub fn new_markham() -> Self {
        use crate::profile::markham;
        Self::new(AudioConfig::new(
            markham::CONTROL_TICK_HZ,
            0,
            DutyMode::Linear,
        ))
    }
}

impl Default for AudioEngine<'static> {
    fn default() -> Self {
        Self::new(AudioConfig::default())
    }
}
