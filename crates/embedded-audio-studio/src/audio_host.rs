//! Host real-time audio playback and synthesis engine using embedded-audio primitives.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use embedded_audio::prelude::*;
use embedded_audio::synth::{
    fm::FmVoice,
    tone::{ToneParams, ToneVoice, Waveform},
    wavetable::{SINE_TABLE, WavetableVoice},
};
use embedded_audio_codegen::{AdsrConfig, DawProject, InstrumentKind, WaveformType};
use std::sync::{Arc, Mutex};

/// Circular buffer capacity for oscilloscope and visualizer streams.
pub const SCOPE_BUFFER_SIZE: usize = 1024;
pub const PWM_BUFFER_SIZE: usize = 1024;
pub const SPECTRUM_FFT_SIZE: usize = 256;

/// Shared visualizer telemetry passed from audio callback to UI.
#[derive(Clone)]
pub struct VisualizerData {
    pub pcm_scope: Vec<f32>,
    pub pwm_pulse_stream: Vec<f32>,
    pub spectrum_mags: Vec<f32>,
    pub current_step: u32,
    pub peak_db: f32,
}

impl Default for VisualizerData {
    fn default() -> Self {
        Self {
            pcm_scope: vec![0.0; SCOPE_BUFFER_SIZE],
            pwm_pulse_stream: vec![0.0; PWM_BUFFER_SIZE],
            spectrum_mags: vec![0.0; SPECTRUM_FFT_SIZE / 2],
            current_step: 0,
            peak_db: -60.0,
        }
    }
}

#[inline(always)]
fn hz_to_phase_inc(hz: u32, sample_rate: u32) -> u32 {
    (((hz as u64) << 32) / (sample_rate as u64).max(1)) as u32
}

/// Authentically pitch-tuned chiptune LFSR noise generator (NES / Game Boy APU style).
#[derive(Clone)]
struct LfsrNoise {
    state: u16,
    phase: u32,
    phase_inc: u32,
    current_sample: i8,
    sub_phase: u32,
    sub_inc: u32,
    sub_gain: i32,
}

impl LfsrNoise {
    fn new() -> Self {
        Self {
            state: 0x7FFF,
            phase: 0,
            phase_inc: 0x8000_0000,
            current_sample: 0,
            sub_phase: 0,
            sub_inc: 0,
            sub_gain: 0,
        }
    }

    fn start(&mut self, note: u8, sample_rate_hz: u32) {
        self.state = 0x7FFF;
        self.phase = 0;

        // Pitch-scale noise clock frequency from 100 Hz (deep rumble) up to 22 kHz (crisp sizzle)
        let noise_clock_hz =
            (120.0 * 2.0_f32.powf((note as f32) / 14.0)).clamp(80.0, 22000.0) as u32;
        self.phase_inc = hz_to_phase_inc(noise_clock_hz, sample_rate_hz);

        // For low notes, add a sub-bass punch oscillator (tuned to the fundamental pitch)
        if note < 55 {
            let base_freq = midi_note_to_freq(note) as u32;
            self.sub_inc = hz_to_phase_inc(base_freq, sample_rate_hz);
            self.sub_gain = ((55 - note as i32) * 5).min(130);
        } else {
            self.sub_gain = 0;
            self.sub_inc = 0;
        }

        self.step_lfsr();
    }

    fn step_lfsr(&mut self) {
        let bit = ((self.state >> 0) ^ (self.state >> 1)) & 1;
        self.state = (self.state >> 1) | (bit << 14);
        self.current_sample = if bit == 1 { 95 } else { -95 };
    }

    fn tick(&mut self) -> i8 {
        let prev_phase = self.phase;
        self.phase = self.phase.wrapping_add(self.phase_inc);
        if self.phase < prev_phase {
            self.step_lfsr();
        }

        let mut out = self.current_sample as i32;

        if self.sub_gain > 0 {
            self.sub_phase = self.sub_phase.wrapping_add(self.sub_inc);
            let idx = (self.sub_phase >> 24) as u8;
            let tri = if idx < 128 {
                (idx as i32 * 2) - 127
            } else {
                127 - ((idx - 128) as i32 * 2)
            };
            out = (out * (255 - self.sub_gain) + tri * self.sub_gain) / 255;
        }

        out.clamp(-127, 127) as i8
    }
}

/// Track playback voice state.
struct ActiveTrackVoice {
    pub track_id: usize,
    tone: ToneVoice,
    fm: FmVoice,
    wavetable: WavetableVoice<'static>,
    noise: Option<LfsrNoise>,
    adsr: Adsr,
    active_kind: usize, // 0 = none, 1 = tone, 2 = fm, 3 = wavetable, 4 = noise
    remaining_ticks: u32,
    velocity_q8: u8,
}

impl ActiveTrackVoice {
    fn new(sample_rate_hz: u32) -> Self {
        let spec = AdsrSpec {
            attack_ms: 5,
            decay_ms: 50,
            sustain_q8: 200,
            release_ms: 80,
        };
        Self {
            track_id: 0,
            tone: ToneVoice::new(),
            fm: FmVoice::new(),
            wavetable: WavetableVoice::sine(),
            noise: None,
            adsr: Adsr::new(spec, sample_rate_hz),
            active_kind: 0,
            remaining_ticks: 0,
            velocity_q8: 255,
        }
    }

    fn trigger(
        &mut self,
        note: u8,
        velocity: u8,
        duration_ticks: u32,
        kind: &InstrumentKind,
        adsr_cfg: &AdsrConfig,
        sample_rate_hz: u32,
    ) {
        self.velocity_q8 = (velocity as u16 * 2).min(255) as u8;
        self.remaining_ticks = duration_ticks;

        let freq_hz = midi_note_to_freq(note) as u32;
        let spec = AdsrSpec {
            attack_ms: adsr_cfg.attack_ms,
            decay_ms: adsr_cfg.decay_ms,
            sustain_q8: adsr_cfg.sustain_q8,
            release_ms: adsr_cfg.release_ms,
        };
        self.adsr = Adsr::new(spec, sample_rate_hz);
        self.adsr.trigger();

        match kind {
            InstrumentKind::Tone { waveform, .. } => {
                let wf = match waveform {
                    WaveformType::Square => Waveform::Square,
                    WaveformType::Triangle => Waveform::Triangle,
                    _ => Waveform::Square,
                };
                let params = ToneParams {
                    freq_hz,
                    duration_ms: 0,
                    waveform: wf,
                    adsr: spec,
                };
                self.tone.start(params, sample_rate_hz);
                self.active_kind = 1;
            }
            InstrumentKind::Fm {
                mod_ratio_x100,
                mod_index_x100,
                ..
            } => {
                let depth = (*mod_index_x100 * 50 / 100).min(255) as u8;
                self.fm
                    .start(freq_hz, *mod_ratio_x100, depth, sample_rate_hz);
                self.active_kind = 2;
            }
            InstrumentKind::Wavetable { .. } => {
                self.wavetable = WavetableVoice::new(&SINE_TABLE);
                self.wavetable.start(freq_hz, sample_rate_hz);
                self.active_kind = 3;
            }
            InstrumentKind::Noise => {
                let mut n = LfsrNoise::new();
                n.start(note, sample_rate_hz);
                self.noise = Some(n);
                self.active_kind = 4;
            }
            InstrumentKind::Sample { .. } => {
                let params = ToneParams {
                    freq_hz,
                    duration_ms: 0,
                    waveform: Waveform::Square,
                    adsr: spec,
                };
                self.tone.start(params, sample_rate_hz);
                self.active_kind = 1;
            }
        }
    }

    fn tick(&mut self) -> i8 {
        if self.remaining_ticks > 0 {
            self.remaining_ticks -= 1;
            if self.remaining_ticks == 0 {
                self.adsr.release();
            }
        }

        self.adsr.tick();
        if !self.adsr.is_active() {
            return 0;
        }
        let env_gain = self.adsr.level_q8();

        let raw_sample: i8 = match self.active_kind {
            1 => self.tone.next_sample().unwrap_or(0),
            2 => self.fm.next_sample().unwrap_or(0),
            3 => self.wavetable.next_sample().unwrap_or(0),
            4 => self.noise.as_mut().map(|n| n.tick()).unwrap_or(0),
            _ => 0,
        };

        // Scale by envelope and note velocity (Q8 fixed-point)
        let scaled =
            ((raw_sample as i32) * (env_gain as i32) / 255 * (self.velocity_q8 as i32) / 255) as i8;
        scaled
    }
}

/// Convert MIDI note number (0..127) to frequency in Hz.
pub fn midi_note_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

/// State of the audio synth player.
pub struct AudioHostState {
    pub is_playing: bool,
    pub is_looping: bool,
    pub current_step: u32,
    pub sample_counter: u32,
    pub samples_per_step: u32,
    pub host_sample_rate_hz: u32,
    pub project: DawProject,
    pub visualizer_data: Arc<Mutex<VisualizerData>>,
    voices: Vec<ActiveTrackVoice>,
    sigma_delta: SigmaDelta,
    scope_ring: Vec<f32>,
    pwm_ring: Vec<f32>,
    fft_in_buf: Vec<f32>,
}

impl AudioHostState {
    pub fn new(
        project: DawProject,
        host_sample_rate_hz: u32,
        visualizer_data: Arc<Mutex<VisualizerData>>,
    ) -> Self {
        let num_tracks = project.tracks.len();
        let samples_per_step = (host_sample_rate_hz as u64 * 60)
            / (project.bpm as u64 * project.steps_per_beat as u64);
        let mut voices = Vec::with_capacity(num_tracks);
        for i in 0..num_tracks {
            let mut v = ActiveTrackVoice::new(host_sample_rate_hz);
            v.track_id = i;
            voices.push(v);
        }

        Self {
            is_playing: false,
            is_looping: true,
            current_step: 0,
            sample_counter: 0,
            samples_per_step: samples_per_step as u32,
            host_sample_rate_hz,
            project,
            visualizer_data,
            voices,
            sigma_delta: SigmaDelta::new(),
            scope_ring: Vec::with_capacity(SCOPE_BUFFER_SIZE),
            pwm_ring: Vec::with_capacity(PWM_BUFFER_SIZE),
            fft_in_buf: Vec::with_capacity(SPECTRUM_FFT_SIZE),
        }
    }

    pub fn set_project(&mut self, project: DawProject) {
        self.project = project;
        self.samples_per_step = ((self.host_sample_rate_hz as u64 * 60)
            / (self.project.bpm as u64 * self.project.steps_per_beat as u64))
            as u32;
        self.voices.clear();
        for i in 0..self.project.tracks.len() {
            let mut v = ActiveTrackVoice::new(self.host_sample_rate_hz);
            v.track_id = i;
            self.voices.push(v);
        }
        self.current_step = 0;
        self.sample_counter = 0;
    }

    pub fn set_bpm(&mut self, bpm: u16) {
        self.project.bpm = bpm.max(30).min(300);
        self.samples_per_step = ((self.host_sample_rate_hz as u64 * 60)
            / (self.project.bpm as u64 * self.project.steps_per_beat as u64))
            as u32;
    }

    pub fn play(&mut self) {
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.current_step = 0;
        self.sample_counter = 0;
    }

    /// Trigger preview note directly (for keyboard / synth lab auditioning).
    pub fn preview_note(&mut self, instrument_idx: usize, note: u8, velocity: u8) {
        if let Some(inst) = self.project.instruments.get(instrument_idx) {
            if let Some(voice) = self.voices.first_mut() {
                let duration_ticks = self.samples_per_step * 4;
                voice.trigger(
                    note,
                    velocity,
                    duration_ticks,
                    &inst.kind,
                    &inst.adsr,
                    self.host_sample_rate_hz,
                );
            }
        }
    }

    /// Trigger a pre-defined sound effect for instant auditioning.
    pub fn preview_sfx(&mut self, sfx_id: usize) {
        match sfx_id {
            0 => {
                // Bootup Chime (C5 -> E5 -> G5 -> C6)
                let kind = InstrumentKind::Fm {
                    mod_ratio_x100: 200,
                    mod_index_x100: 160,
                    feedback_x100: 10,
                };
                let adsr = AdsrConfig {
                    attack_ms: 2,
                    decay_ms: 180,
                    sustain_q8: 100,
                    release_ms: 200,
                };
                if let Some(voice) = self.voices.first_mut() {
                    voice.trigger(
                        84,
                        127,
                        self.samples_per_step * 3,
                        &kind,
                        &adsr,
                        self.host_sample_rate_hz,
                    );
                }
            }
            1 => {
                // Laser Pew (Fast downward chirp)
                let kind = InstrumentKind::Tone {
                    waveform: WaveformType::Square,
                    duty: 128,
                };
                let adsr = AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 40,
                    sustain_q8: 0,
                    release_ms: 20,
                };
                if let Some(voice) = self.voices.first_mut() {
                    voice.trigger(
                        88,
                        127,
                        self.samples_per_step,
                        &kind,
                        &adsr,
                        self.host_sample_rate_hz,
                    );
                }
            }
            2 => {
                // Shutdown (Power-down droop)
                let kind = InstrumentKind::Tone {
                    waveform: WaveformType::Sawtooth,
                    duty: 128,
                };
                let adsr = AdsrConfig {
                    attack_ms: 5,
                    decay_ms: 250,
                    sustain_q8: 0,
                    release_ms: 200,
                };
                if let Some(voice) = self.voices.first_mut() {
                    voice.trigger(
                        48,
                        120,
                        self.samples_per_step * 4,
                        &kind,
                        &adsr,
                        self.host_sample_rate_hz,
                    );
                }
            }
            3 => {
                // Coin Collect (B5 -> E6)
                let kind = InstrumentKind::Tone {
                    waveform: WaveformType::Square,
                    duty: 128,
                };
                let adsr = AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 80,
                    sustain_q8: 0,
                    release_ms: 60,
                };
                if let Some(voice) = self.voices.first_mut() {
                    voice.trigger(
                        88,
                        125,
                        self.samples_per_step * 2,
                        &kind,
                        &adsr,
                        self.host_sample_rate_hz,
                    );
                }
            }
            4 => {
                // Power-Up Fanfare
                let kind = InstrumentKind::Fm {
                    mod_ratio_x100: 200,
                    mod_index_x100: 150,
                    feedback_x100: 20,
                };
                let adsr = AdsrConfig {
                    attack_ms: 2,
                    decay_ms: 120,
                    sustain_q8: 120,
                    release_ms: 150,
                };
                if let Some(voice) = self.voices.first_mut() {
                    voice.trigger(
                        77,
                        127,
                        self.samples_per_step * 3,
                        &kind,
                        &adsr,
                        self.host_sample_rate_hz,
                    );
                }
            }
            5 => {
                // Explosion / Hit
                let kind = InstrumentKind::Noise;
                let adsr = AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 160,
                    sustain_q8: 0,
                    release_ms: 80,
                };
                if let Some(voice) = self.voices.first_mut() {
                    voice.trigger(
                        60,
                        127,
                        self.samples_per_step * 3,
                        &kind,
                        &adsr,
                        self.host_sample_rate_hz,
                    );
                }
            }
            6 => {
                // Error Alert Buzz
                let kind = InstrumentKind::Tone {
                    waveform: WaveformType::Square,
                    duty: 128,
                };
                let adsr = AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 80,
                    sustain_q8: 200,
                    release_ms: 40,
                };
                if let Some(voice) = self.voices.first_mut() {
                    voice.trigger(
                        48,
                        127,
                        self.samples_per_step * 2,
                        &kind,
                        &adsr,
                        self.host_sample_rate_hz,
                    );
                }
            }
            7 => {
                // Jump / Spring
                let kind = InstrumentKind::Tone {
                    waveform: WaveformType::Triangle,
                    duty: 128,
                };
                let adsr = AdsrConfig {
                    attack_ms: 2,
                    decay_ms: 100,
                    sustain_q8: 100,
                    release_ms: 50,
                };
                if let Some(voice) = self.voices.first_mut() {
                    voice.trigger(
                        72,
                        120,
                        self.samples_per_step * 2,
                        &kind,
                        &adsr,
                        self.host_sample_rate_hz,
                    );
                }
            }
            _ => {}
        }
    }

    /// Generate next single mono audio sample.
    pub fn next_sample(&mut self) -> f32 {
        if self.is_playing {
            // Check step boundaries
            if self.sample_counter == 0 {
                let has_solo = self.project.tracks.iter().any(|t| t.solo);

                // Trigger any notes scheduled on this step
                for (track_idx, track) in self.project.tracks.iter().enumerate() {
                    let should_trigger = if has_solo {
                        track.solo && !track.muted
                    } else {
                        !track.muted
                    };

                    if !should_trigger {
                        continue;
                    }

                    for note_ev in &track.notes {
                        if note_ev.step == self.current_step {
                            if let Some(inst) = self.project.instruments.get(track.instrument_id) {
                                if let Some(voice) = self.voices.get_mut(track_idx) {
                                    let duration_ticks =
                                        note_ev.duration_steps * self.samples_per_step;
                                    voice.trigger(
                                        note_ev.note,
                                        note_ev.velocity,
                                        duration_ticks,
                                        &inst.kind,
                                        &inst.adsr,
                                        self.host_sample_rate_hz,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            self.sample_counter += 1;
            if self.sample_counter >= self.samples_per_step {
                self.sample_counter = 0;
                self.current_step += 1;
                if self.current_step >= self.project.total_steps {
                    if self.is_looping {
                        self.current_step = 0;
                    } else {
                        self.is_playing = false;
                        self.current_step = 0;
                    }
                }
            }
        }

        // Mix all active (unmuted / soloed) track voices
        let has_solo = self.project.tracks.iter().any(|t| t.solo);
        let mut mixed: i32 = 0;
        for (i, voice) in self.voices.iter_mut().enumerate() {
            let sample = voice.tick();
            if let Some(track) = self.project.tracks.get(i) {
                let should_mix = if has_solo {
                    track.solo && !track.muted
                } else {
                    !track.muted
                };

                if should_mix {
                    let scaled = (sample as i32 * track.volume_q8 as i32) / 255;
                    mixed += scaled;
                }
            }
        }

        let mixed_clamped = mixed.clamp(-128, 127) as i8;
        let float_sample = mixed_clamped as f32 / 128.0;

        // Simulate 1-bit Sigma-Delta / PWM duty output
        let shaped = self.sigma_delta.shape(mixed_clamped);
        let pwm_val = if shaped > 0 { 1.0 } else { -1.0 };

        // Push to visualization rings
        if self.scope_ring.len() < SCOPE_BUFFER_SIZE {
            self.scope_ring.push(float_sample);
        } else {
            self.scope_ring.remove(0);
            self.scope_ring.push(float_sample);
        }

        if self.pwm_ring.len() < PWM_BUFFER_SIZE {
            self.pwm_ring.push(pwm_val);
        } else {
            self.pwm_ring.remove(0);
            self.pwm_ring.push(pwm_val);
        }

        self.fft_in_buf.push(float_sample);
        if self.fft_in_buf.len() >= SPECTRUM_FFT_SIZE {
            // Update visualizer mutex
            if let Ok(mut vis) = self.visualizer_data.try_lock() {
                vis.pcm_scope = self.scope_ring.clone();
                vis.pwm_pulse_stream = self.pwm_ring.clone();
                vis.current_step = self.current_step;
                let peak = float_sample.abs();
                vis.peak_db = if peak > 0.0001 {
                    20.0 * peak.log10()
                } else {
                    -60.0
                };

                // Fast approximate spectrum magnitudes
                let mut mags = vec![0.0f32; SPECTRUM_FFT_SIZE / 2];
                for i in 0..SPECTRUM_FFT_SIZE / 2 {
                    let freq_bin = (i + 1) as f32;
                    let mut sum_cos = 0.0;
                    let mut sum_sin = 0.0;
                    for (n, &s) in self.fft_in_buf.iter().enumerate() {
                        let angle = 2.0 * std::f32::consts::PI * freq_bin * (n as f32)
                            / (SPECTRUM_FFT_SIZE as f32);
                        sum_cos += s * angle.cos();
                        sum_sin += s * angle.sin();
                    }
                    mags[i] = ((sum_cos * sum_cos + sum_sin * sum_sin).sqrt()
                        / (SPECTRUM_FFT_SIZE as f32 / 2.0))
                        .min(1.0);
                }
                vis.spectrum_mags = mags;
            }
            self.fft_in_buf.clear();
        }

        float_sample
    }
}

/// Host audio manager with cpal stream handle.
pub struct HostAudioDevice {
    pub state: Arc<Mutex<AudioHostState>>,
    #[allow(dead_code)]
    pub stream: Option<cpal::Stream>,
}

impl HostAudioDevice {
    pub fn new(project: DawProject) -> (Self, Arc<Mutex<VisualizerData>>) {
        let visualizer_data = Arc::new(Mutex::new(VisualizerData::default()));
        let (stream, host_sample_rate) = Self::init_cpal_stream();

        let state = Arc::new(Mutex::new(AudioHostState::new(
            project,
            host_sample_rate,
            Arc::clone(&visualizer_data),
        )));

        // Re-attach state to the live stream callback
        let state_for_stream = Arc::clone(&state);
        let active_stream = if let Some((device, config)) = Self::get_device_and_config() {
            Self::build_stream(&device, &config, state_for_stream)
        } else {
            None
        };

        // Suppress unused stream warning by using active_stream
        let _ = stream;

        (
            Self {
                state,
                stream: active_stream,
            },
            visualizer_data,
        )
    }

    fn get_device_and_config() -> Option<(cpal::Device, cpal::SupportedStreamConfig)> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;
        Some((device, config))
    }

    fn init_cpal_stream() -> (Option<cpal::Stream>, u32) {
        if let Some((_, config)) = Self::get_device_and_config() {
            let sample_rate = config.sample_rate().0;
            (None, sample_rate)
        } else {
            (None, 44100)
        }
    }

    fn build_stream(
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        state: Arc<Mutex<AudioHostState>>,
    ) -> Option<cpal::Stream> {
        let channels = config.channels() as usize;
        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream_res = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.clone().into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if let Ok(mut state_guard) = state.lock() {
                        for frame in data.chunks_mut(channels) {
                            let sample = state_guard.next_sample();
                            for out_sample in frame.iter_mut() {
                                *out_sample = sample;
                            }
                        }
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config.clone().into(),
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    if let Ok(mut state_guard) = state.lock() {
                        for frame in data.chunks_mut(channels) {
                            let sample = state_guard.next_sample();
                            let s16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                            for out_sample in frame.iter_mut() {
                                *out_sample = s16;
                            }
                        }
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_output_stream(
                &config.clone().into(),
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    if let Ok(mut state_guard) = state.lock() {
                        for frame in data.chunks_mut(channels) {
                            let sample = state_guard.next_sample();
                            let u16_val =
                                ((sample * 0.5 + 0.5) * 65535.0).clamp(0.0, 65535.0) as u16;
                            for out_sample in frame.iter_mut() {
                                *out_sample = u16_val;
                            }
                        }
                    }
                },
                err_fn,
                None,
            ),
            _ => return None,
        };

        if let Ok(s) = stream_res {
            let _ = s.play();
            Some(s)
        } else {
            None
        }
    }
}
