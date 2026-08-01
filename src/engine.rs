use crate::bank::{EffectEntry, SoundBank};
use crate::config::{AudioConfig, crossfade_step_q8};
use crate::envelope::AdsrSpec;
use crate::error::AudioError;
use crate::fixed::{apply_gain_q8, mix_crossfade};
use crate::output::{DutyMode, PwmMapper, limit_bus};
use crate::voice::Voice;

#[cfg(feature = "fm")]
use crate::output::{FmMapper, FmTick};

/// Policy for dynamic voice allocation when triggering new effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceStealingPolicy {
    /// Steal the lowest-priority voice if all voices are active and new priority is higher.
    #[default]
    LowestPriorityOldest,
    /// Only allocate if a voice channel is completely free.
    FreeChannelOnly,
}

/// N-voice mixer with bank playback, dynamic voice allocation, and duty-modulated PWM output.
pub struct AudioEngine<'a, const N: usize = 2> {
    bank: Option<SoundBank<'a>>,
    config: AudioConfig,
    voices: [Voice<'a>; N],
    mapper: PwmMapper,
    crossfade_t_q8: u8,
    crossfade_step_q8: u8,
    crossfade_active: bool,
    stealing_policy: VoiceStealingPolicy,
    #[cfg(feature = "fm")]
    fm_mapper: FmMapper,
}

impl<'a> AudioEngine<'a, 2> {
    /// Create a standard 2-voice audio engine.
    pub fn new(config: AudioConfig) -> Self {
        Self::with_voice_count(config)
    }

    pub fn from_sample_rate(sample_rate_hz: u32, pwm_period: u16, duty_mode: DutyMode) -> Self {
        Self::new(AudioConfig::new(sample_rate_hz, pwm_period, duty_mode))
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

impl<'a, const N: usize> AudioEngine<'a, N> {
    /// Create an audio engine with generic voice count N.
    pub fn with_voice_count(config: AudioConfig) -> Self {
        let rate = config.sample_rate_hz;
        Self {
            bank: None,
            config,
            voices: [Voice::silent(rate); N],
            mapper: PwmMapper::new(config.duty_mode),
            crossfade_t_q8: 0,
            crossfade_step_q8: 0,
            crossfade_active: false,
            stealing_policy: VoiceStealingPolicy::default(),
            #[cfg(feature = "fm")]
            fm_mapper: FmMapper::markham(),
        }
    }

    pub fn config(&self) -> AudioConfig {
        self.config
    }

    pub fn set_stealing_policy(&mut self, policy: VoiceStealingPolicy) {
        self.stealing_policy = policy;
    }

    pub fn stealing_policy(&self) -> VoiceStealingPolicy {
        self.stealing_policy
    }

    pub fn voice_count(&self) -> usize {
        N
    }

    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|v| v.is_audible()).count()
    }

    pub fn voice(&self, idx: usize) -> Option<&Voice<'a>> {
        self.voices.get(idx)
    }

    pub fn voice_mut(&mut self, idx: usize) -> Option<&mut Voice<'a>> {
        self.voices.get_mut(idx)
    }

    pub fn set_bank(&mut self, bank: SoundBank<'a>) {
        let rate = bank.sample_rate_hz;
        self.config.sample_rate_hz = rate;
        self.voices = [Voice::silent(rate); N];
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

    /// Dynamically allocate a voice channel based on priority and stealing policy.
    pub fn allocate_voice(&mut self, priority: u8) -> Option<usize> {
        if N == 0 {
            return None;
        }
        // 1. Look for an inaudible / idle voice
        for (i, v) in self.voices.iter().enumerate() {
            if !v.is_audible() {
                return Some(i);
            }
        }
        // 2. Check stealing policy
        if self.stealing_policy == VoiceStealingPolicy::LowestPriorityOldest {
            let mut lowest_idx = None;
            let mut lowest_prio = priority;
            for (i, v) in self.voices.iter().enumerate() {
                if v.priority < lowest_prio {
                    lowest_prio = v.priority;
                    lowest_idx = Some(i);
                }
            }
            return lowest_idx;
        }
        None
    }

    /// Play effect on an automatically allocated voice channel.
    pub fn play(&mut self, effect_id: u16, adsr: AdsrSpec) -> Result<usize, AudioError> {
        self.play_with_priority(effect_id, adsr, 128)
    }

    /// Play with custom priority on an allocated voice channel.
    pub fn play_with_priority(
        &mut self,
        effect_id: u16,
        adsr: AdsrSpec,
        priority: u8,
    ) -> Result<usize, AudioError> {
        let idx = self.allocate_voice(priority).ok_or(AudioError::VoiceBusy)?;
        let bank = self.bank.ok_or(AudioError::NoBank)?;
        let entry = bank.find_by_id(effect_id)?;
        self.start_on_voice(idx, &bank, entry, adsr, priority)?;
        Ok(idx)
    }

    /// Crossfade voice 0 → `effect_id` on voice 1 over `duration_ms`.
    pub fn crossfade_to(
        &mut self,
        effect_id: u16,
        duration_ms: u16,
        adsr: AdsrSpec,
    ) -> Result<(), AudioError> {
        if N < 2 {
            return Err(AudioError::VoiceBusy);
        }
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
        if !self.crossfade_active || N < 2 {
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

    pub fn start_on_voice(
        &mut self,
        idx: usize,
        bank: &SoundBank<'a>,
        entry: EffectEntry,
        adsr: AdsrSpec,
        priority: u8,
    ) -> Result<(), AudioError> {
        if idx >= N {
            return Err(AudioError::VoiceBusy);
        }
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

        if N == 0 {
            return 0;
        }

        if self.crossfade_active && N >= 2 {
            let sa = self.voices[0].next_sample();
            let sb = self.voices[1].next_sample();
            let mixed = match (sa, sb) {
                (None, None) => 0,
                (Some(s), None) => s as i32,
                (None, Some(s)) => s as i32,
                (Some(sa), Some(sb)) => mix_crossfade(sa, sb, self.crossfade_t_q8) as i32,
            };
            return apply_gain_q8(limit_bus(mixed), self.config.master_gain_q8);
        }

        let mut sum: i32 = 0;
        let mut active_count: i32 = 0;
        for v in &mut self.voices {
            if let Some(s) = v.next_sample() {
                sum += s as i32;
                active_count += 1;
            }
        }

        let mixed = if active_count > 1 {
            sum / active_count
        } else {
            sum
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
        if N > 0 {
            self.voices[0] = Voice::silent(rate);
            self.voices[0]
                .source
                .start_tone(freq_hz, duration_ms, waveform, rate);
            self.voices[0].trigger_adsr(AdsrSpec::click());
        }
    }

    #[cfg(feature = "fm")]
    pub fn set_fm_mapper(&mut self, mapper: FmMapper) {
        self.fm_mapper = mapper;
    }

    #[cfg(feature = "fm")]
    #[allow(clippy::collapsible_if)]
    /// FM buzzer backend (optional; not used for duty-modulation products).
    pub fn tick_fm(&mut self) -> FmTick {
        let pcm = self.tick_mixed_pcm();
        let rate = self.config.sample_rate_hz;

        if !self.crossfade_active && N > 0 {
            if let Some(hz) = self.voices[0].source.carrier_hz(rate) {
                return FmMapper::from_carrier(hz, self.voices[0].source.is_active());
            }
        }

        self.fm_mapper.map_pcm(pcm)
    }
}

impl Default for AudioEngine<'static, 2> {
    fn default() -> Self {
        Self::new(AudioConfig::default())
    }
}
