//! Procedural analog-style drum synthesis, ported from DaisySP's `Drums` module (in turn ported
//! from Emilie Gillet's Mutable Instruments Plaits drum models). Kick, snare, and hi-hat are
//! generated entirely from short trigger pulses driving resonant filters and noise — no sample
//! memory at all, which fits this crate's "avoid flash-hungry PCM" philosophy.
//!
//! Requires the `dsp` feature for `libm`/`std` trig (`embedded_dsp::FloatMath`). Output is `f32`
//! in roughly `-1.0..=1.0`; convert with `(sample * 127.0).clamp(-128.0, 127.0) as i8` to mix
//! into an `i8` PCM bus.
//!
//! Like DaisySP's originals, the resonant filters inside these models are only guaranteed
//! stable for coefficients held steady (or changed gradually) between samples — call the
//! `set_*` methods once per hit (e.g. on `trigger`), not every sample with wildly different
//! values, or the resonators can build up energy and diverge.

#[allow(unused_imports)]
use embedded_dsp::FloatMath;
use embedded_dsp::StateVariableFilter;

const ONE_TWELFTH: f32 = 1.0 / 12.0;

fn semitones_to_ratio(semitones: f32) -> f32 {
    2.0f32.powf(semitones * ONE_TWELFTH)
}

/// Asymmetric diode clipper used by the bass drum's exciter path.
fn diode(x: f32) -> f32 {
    if x >= 0.0 {
        x
    } else {
        let y = x * 2.0;
        0.7 * y / (1.0 + y.abs())
    }
}

fn soft_limit(x: f32) -> f32 {
    x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
}

fn soft_clip(x: f32) -> f32 {
    if x < -3.0 {
        -1.0
    } else if x > 3.0 {
        1.0
    } else {
        soft_limit(x)
    }
}

fn one_pole(state: &mut f32, input: f32, coeff: f32) {
    *state += coeff * (input - *state);
}

/// xorshift32 PRNG for drum noise excitation (no external dependency, no_std-friendly).
#[derive(Debug, Clone, Copy)]
struct Noise(u32);

impl Noise {
    const fn new() -> Self {
        Self(0x1234_5678)
    }

    /// Next value in `0.0..=1.0`.
    fn next_unipolar(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16_777_216.0
    }
}

/// 808-style analog bass drum, ported from `daisysp::AnalogBassDrum`.
#[derive(Debug, Clone)]
pub struct AnalogBassDrum {
    sample_rate_hz: f32,

    accent: f32,
    f0: f32,
    tone: f32,
    decay: f32,
    attack_fm_amount: f32,
    self_fm_amount: f32,
    sustain: bool,

    pulse_remaining_samples: i32,
    fm_pulse_remaining_samples: i32,
    pulse: f32,
    pulse_height: f32,
    pulse_lp: f32,
    fm_pulse_lp: f32,
    retrig_pulse: f32,
    lp_out: f32,
    tone_lp: f32,

    resonator: StateVariableFilter,
    phase: f32,
}

impl AnalogBassDrum {
    /// Creates a bass drum at 50 Hz with light accent, tone, and decay defaults.
    pub fn new(sample_rate_hz: f32) -> Self {
        let mut bd = Self {
            sample_rate_hz,
            accent: 0.1,
            f0: 0.0,
            tone: 0.1,
            decay: -0.07,
            attack_fm_amount: 25.0,
            self_fm_amount: 50.0,
            sustain: false,
            pulse_remaining_samples: 0,
            fm_pulse_remaining_samples: 0,
            pulse: 0.0,
            pulse_height: 0.0,
            pulse_lp: 0.0,
            fm_pulse_lp: 0.0,
            retrig_pulse: 0.0,
            lp_out: 0.0,
            tone_lp: 0.0,
            resonator: StateVariableFilter::new(sample_rate_hz),
            phase: 0.0,
        };
        bd.set_freq(50.0);
        bd
    }

    /// Plays infinitely at the drum's resonant pitch instead of decaying (for tuning/preview).
    pub fn set_sustain(&mut self, sustain: bool) {
        self.sustain = sustain;
    }

    /// Sets the strike accent, `0.0..=1.0`.
    pub fn set_accent(&mut self, accent: f32) {
        self.accent = accent.clamp(0.0, 1.0);
    }

    /// Sets the drum's root frequency in Hz.
    pub fn set_freq(&mut self, freq_hz: f32) {
        self.f0 = (freq_hz / self.sample_rate_hz).clamp(0.0, 0.5);
    }

    /// Sets the amount of transient "click", `0.0..=1.0`.
    pub fn set_tone(&mut self, tone: f32) {
        self.tone = tone.clamp(0.0, 1.0);
    }

    /// Sets the decay length, best in `0.0..=1.0`.
    pub fn set_decay(&mut self, decay: f32) {
        self.decay = decay * 0.1 - 0.1;
    }

    /// Sets the pitch-attack FM amount, best in `0.0..=1.0`.
    pub fn set_attack_fm_amount(&mut self, amount: f32) {
        self.attack_fm_amount = amount * 50.0;
    }

    /// Sets the self-FM amount (also affects attack FM and volume decay), best in `0.0..=1.0`.
    pub fn set_self_fm_amount(&mut self, amount: f32) {
        self.self_fm_amount = amount * 50.0;
    }

    /// Strikes the drum on the next [`Self::process`] call.
    pub fn trigger(&mut self) {
        let trigger_pulse_duration = (1.0e-3 * self.sample_rate_hz) as i32;
        let fm_pulse_duration = (6.0e-3 * self.sample_rate_hz) as i32;
        self.pulse_remaining_samples = trigger_pulse_duration;
        self.fm_pulse_remaining_samples = fm_pulse_duration;
        self.pulse_height = 3.0 + 7.0 * self.accent;
        self.lp_out = 0.0;
    }

    /// Generates the next sample. `trigger` strikes the drum on this call.
    pub fn process(&mut self, trigger: bool) -> f32 {
        if trigger {
            self.trigger();
        }

        let pulse_decay_time = 0.2e-3 * self.sample_rate_hz;
        let pulse_filter_time = 0.1e-3 * self.sample_rate_hz;
        let retrig_pulse_duration = 0.05 * self.sample_rate_hz;

        let scale = 0.001 / self.f0.max(1.0e-9);
        let q = 1500.0 * semitones_to_ratio(self.decay * 80.0);
        let tone_f = (4.0 * self.f0 * semitones_to_ratio(self.tone * 108.0)).min(1.0);
        let exciter_leak = 0.08 * (self.tone + 0.25);

        let mut pulse;
        if self.pulse_remaining_samples > 0 {
            self.pulse_remaining_samples -= 1;
            pulse = if self.pulse_remaining_samples > 0 {
                self.pulse_height
            } else {
                self.pulse_height - 1.0
            };
            self.pulse = pulse;
        } else {
            self.pulse *= 1.0 - 1.0 / pulse_decay_time;
            pulse = self.pulse;
        }
        if self.sustain {
            pulse = 0.0;
        }

        one_pole(&mut self.pulse_lp, pulse, 1.0 / pulse_filter_time);
        pulse = diode((pulse - self.pulse_lp) + pulse * 0.044);

        let mut fm_pulse;
        if self.fm_pulse_remaining_samples > 0 {
            self.fm_pulse_remaining_samples -= 1;
            fm_pulse = 1.0;
            self.retrig_pulse = if self.fm_pulse_remaining_samples > 0 {
                0.0
            } else {
                -0.8
            };
        } else {
            fm_pulse = 0.0;
            self.retrig_pulse *= 1.0 - 1.0 / retrig_pulse_duration;
        }
        if self.sustain {
            fm_pulse = 0.0;
        }
        one_pole(&mut self.fm_pulse_lp, fm_pulse, 1.0 / pulse_filter_time);

        let punch = 0.7 + diode(10.0 * self.lp_out - 1.0);
        let attack_fm = self.fm_pulse_lp * 1.7 * self.attack_fm_amount;
        let self_fm = punch * 0.08 * self.self_fm_amount;
        let f = (self.f0 * (1.0 + attack_fm + self_fm)).clamp(0.0, 0.4);

        let resonator_out;
        if self.sustain {
            let sustain_gain = self.accent * self.decay;
            self.phase += f;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            resonator_out = (core::f32::consts::TAU * self.phase).sin() * sustain_gain;
            self.lp_out = (core::f32::consts::TAU * self.phase).cos() * sustain_gain;
        } else {
            self.resonator.set_cutoff(f * self.sample_rate_hz);
            self.resonator.set_resonance(0.4 * q * f);
            self.resonator
                .process((pulse - self.retrig_pulse * 0.2) * scale);
            resonator_out = self.resonator.band();
            self.lp_out = self.resonator.low();
        }

        one_pole(&mut self.tone_lp, pulse * exciter_leak + resonator_out, tone_f);
        self.tone_lp
    }
}

/// Number of resonant modes making up the analog snare's "shell" tone.
const SNARE_NUM_MODES: usize = 5;
const SNARE_MODE_FREQUENCIES: [f32; SNARE_NUM_MODES] = [1.00, 2.00, 3.18, 4.16, 5.62];

/// 808-style analog snare drum, ported from `daisysp::AnalogSnareDrum`.
#[derive(Debug, Clone)]
pub struct AnalogSnareDrum {
    sample_rate_hz: f32,

    f0: f32,
    tone: f32,
    accent: f32,
    snappy: f32,
    decay: f32,
    sustain: bool,

    pulse_remaining_samples: i32,
    pulse: f32,
    pulse_height: f32,
    pulse_lp: f32,
    noise_envelope: f32,

    resonators: [StateVariableFilter; SNARE_NUM_MODES],
    noise_filter: StateVariableFilter,
    phases: [f32; SNARE_NUM_MODES],
    noise: Noise,
}

impl AnalogSnareDrum {
    /// Creates a snare drum at 200 Hz with a snappy 808-style default voicing.
    pub fn new(sample_rate_hz: f32) -> Self {
        let mut sd = Self {
            sample_rate_hz,
            f0: 0.0,
            tone: 1.0,
            accent: 0.6,
            snappy: 0.7,
            decay: 0.3,
            sustain: false,
            pulse_remaining_samples: 0,
            pulse: 0.0,
            pulse_height: 0.0,
            pulse_lp: 0.0,
            noise_envelope: 0.0,
            resonators: [StateVariableFilter::new(sample_rate_hz); SNARE_NUM_MODES],
            noise_filter: StateVariableFilter::new(sample_rate_hz),
            phases: [0.0; SNARE_NUM_MODES],
            noise: Noise::new(),
        };
        sd.set_freq(200.0);
        sd
    }

    /// Plays infinitely instead of decaying (for tuning/preview).
    pub fn set_sustain(&mut self, sustain: bool) {
        self.sustain = sustain;
    }

    /// Sets the strike accent, `0.0..=1.0`.
    pub fn set_accent(&mut self, accent: f32) {
        self.accent = accent.clamp(0.0, 1.0);
    }

    /// Sets the drum's root frequency in Hz.
    pub fn set_freq(&mut self, freq_hz: f32) {
        self.f0 = (freq_hz / self.sample_rate_hz).clamp(0.0, 0.4);
    }

    /// Sets the shell brightness, `0.0` (dark, 808-style) to `1.0` (bright, extra modes).
    pub fn set_tone(&mut self, tone: f32) {
        self.tone = tone.clamp(0.0, 1.0) * 2.0;
    }

    /// Sets the decay length (positive values).
    pub fn set_decay(&mut self, decay: f32) {
        self.decay = decay.max(0.0);
    }

    /// Sets the snare/shell mix, `1.0` = all snare noise, `0.0` = all resonant shell.
    pub fn set_snappy(&mut self, snappy: f32) {
        self.snappy = snappy.clamp(0.0, 1.0);
    }

    /// Strikes the drum on the next [`Self::process`] call.
    pub fn trigger(&mut self) {
        self.pulse_remaining_samples = (1.0e-3 * self.sample_rate_hz) as i32;
        self.pulse_height = 3.0 + 7.0 * self.accent;
        self.noise_envelope = 2.0;
    }

    /// Generates the next sample. `trigger` strikes the drum on this call.
    pub fn process(&mut self, trigger: bool) -> f32 {
        if trigger {
            self.trigger();
        }

        let decay_xt = self.decay * (1.0 + self.decay * (self.decay - 1.0));
        let pulse_decay_time = 0.1e-3 * self.sample_rate_hz;
        let q = 2000.0 * semitones_to_ratio(decay_xt * 84.0);
        let noise_envelope_decay =
            1.0 - 0.0017 * semitones_to_ratio(-self.decay * (50.0 + self.snappy * 10.0));
        let exciter_leak = self.snappy * (2.0 - self.snappy) * 0.1;
        let snappy = (self.snappy * 1.1 - 0.05).clamp(0.0, 1.0);

        let mut tone = self.tone;

        let mut f = [0.0f32; SNARE_NUM_MODES];
        let mut gain = [0.0f32; SNARE_NUM_MODES];
        for i in 0..SNARE_NUM_MODES {
            f[i] = (self.f0 * SNARE_MODE_FREQUENCIES[i]).min(0.499);
            self.resonators[i].set_cutoff(f[i] * self.sample_rate_hz);
            let mode_q = if i == 0 { q } else { q * 0.25 };
            self.resonators[i].set_resonance(f[i] * mode_q * 0.2);
        }

        if tone < 0.666_667 {
            tone *= 1.5;
            gain[0] = 1.5 + (1.0 - tone) * (1.0 - tone) * 4.5;
            gain[1] = 2.0 * tone + 0.15;
        } else {
            tone = (tone - 0.666_667) * 3.0;
            gain[0] = 1.5 - tone * 0.5;
            gain[1] = 2.15 - tone * 0.7;
            for g in gain.iter_mut().skip(2) {
                *g = tone;
                tone *= tone;
            }
        }

        let f_noise = (self.f0 * 16.0).clamp(0.0, 0.499);
        self.noise_filter.set_cutoff(f_noise * self.sample_rate_hz);
        self.noise_filter.set_resonance(f_noise * 1.5);

        let pulse;
        if self.pulse_remaining_samples > 0 {
            self.pulse_remaining_samples -= 1;
            pulse = if self.pulse_remaining_samples > 0 {
                self.pulse_height
            } else {
                self.pulse_height - 1.0
            };
            self.pulse = pulse;
        } else {
            self.pulse *= 1.0 - 1.0 / pulse_decay_time;
            pulse = self.pulse;
        }

        let sustain_gain = self.accent * self.decay;
        // Original: `fclamp(pulse_lp_, pulse, 0.75f)` == `fmin(fmax(pulse_lp_, pulse), 0.75)`.
        self.pulse_lp = self.pulse_lp.max(pulse).min(0.75);

        let mut shell = 0.0f32;
        for i in 0..SNARE_NUM_MODES {
            let excitation = if i == 0 {
                (pulse - self.pulse_lp) + 0.006 * pulse
            } else {
                0.026 * pulse
            };

            self.phases[i] += f[i];
            if self.phases[i] >= 1.0 {
                self.phases[i] -= 1.0;
            }

            self.resonators[i].process(excitation);

            shell += gain[i]
                * if self.sustain {
                    (self.phases[i] * core::f32::consts::TAU).sin() * sustain_gain * 0.25
                } else {
                    self.resonators[i].band() + excitation * exciter_leak
                };
        }
        shell = soft_clip(shell);

        let mut noise = (2.0 * self.noise.next_unipolar() - 1.0).max(0.0);
        self.noise_envelope *= noise_envelope_decay;
        noise *= (if self.sustain { sustain_gain } else { self.noise_envelope }) * snappy * 2.0;

        self.noise_filter.process(noise);
        let filtered_noise = self.noise_filter.band();

        filtered_noise + shell * (1.0 - snappy)
    }
}

/// Six-oscillator "metallic noise" source used by [`HiHat`], ported from `daisysp::SquareNoise`.
#[derive(Debug, Clone, Copy)]
struct SquareNoise {
    phase: [u32; 6],
}

const SQUARE_NOISE_RATIOS: [f32; 6] = [1.0, 1.304, 1.466, 1.787, 1.932, 2.536];

impl SquareNoise {
    const fn new() -> Self {
        Self { phase: [0; 6] }
    }

    fn process(&mut self, f0: f32) -> f32 {
        let mut noise: u32 = 0;
        for (phase, ratio) in self.phase.iter_mut().zip(SQUARE_NOISE_RATIOS) {
            let f = (f0 * ratio).min(0.499);
            let increment = (f * 4_294_967_296.0) as u32;
            *phase = phase.wrapping_add(increment);
            noise += *phase >> 31;
        }
        0.33 * noise as f32 - 1.0
    }
}

/// 808-style hi-hat: six ring-oscillator "metallic noise" plus a resonant bandpass and a variable
/// mix of clocked noise, ported from `daisysp::HiHat` (the default `SquareNoise` + linear-VCA
/// configuration).
#[derive(Debug, Clone)]
pub struct HiHat {
    sample_rate_hz: f32,

    accent: f32,
    f0: f32,
    tone: f32,
    decay: f32,
    noisiness: f32,
    sustain: bool,

    envelope: f32,
    noise_clock: f32,
    noise_sample: f32,

    metallic_noise: SquareNoise,
    noise_coloration: StateVariableFilter,
    hpf: StateVariableFilter,
    noise: Noise,
}

impl HiHat {
    /// Creates a hi-hat at 3 kHz with a fast, snappy default decay.
    pub fn new(sample_rate_hz: f32) -> Self {
        let mut hh = Self {
            sample_rate_hz,
            accent: 0.8,
            f0: 0.0,
            tone: 0.5,
            decay: 0.0,
            noisiness: 0.64,
            sustain: false,
            envelope: 0.0,
            noise_clock: 0.0,
            noise_sample: 0.0,
            metallic_noise: SquareNoise::new(),
            noise_coloration: StateVariableFilter::new(sample_rate_hz),
            hpf: StateVariableFilter::new(sample_rate_hz),
            noise: Noise::new(),
        };
        hh.set_freq(3000.0);
        hh.set_decay(0.2);
        hh
    }

    /// Rings out infinitely instead of decaying (for tuning/preview).
    pub fn set_sustain(&mut self, sustain: bool) {
        self.sustain = sustain;
    }

    /// Sets the strike accent, `0.0..=1.0`.
    pub fn set_accent(&mut self, accent: f32) {
        self.accent = accent.clamp(0.0, 1.0);
    }

    /// Sets the hi-hat's root frequency in Hz.
    pub fn set_freq(&mut self, freq_hz: f32) {
        self.f0 = (freq_hz / self.sample_rate_hz).clamp(0.0, 1.0);
    }

    /// Sets overall brightness, `0.0` (dark) to `1.0` (bright).
    pub fn set_tone(&mut self, tone: f32) {
        self.tone = tone.clamp(0.0, 1.0);
    }

    /// Sets the decay length (positive values; tuned for `0.0..=1.0`).
    pub fn set_decay(&mut self, decay: f32) {
        self.decay = decay.max(0.0) * 1.7 - 1.2;
    }

    /// Sets the tone/noise mix, `1.0` = all clocked noise, `0.0` = all metallic oscillator tone.
    pub fn set_noisiness(&mut self, noisiness: f32) {
        let n = noisiness.clamp(0.0, 1.0);
        self.noisiness = n * n;
    }

    /// Strikes the hi-hat on the next [`Self::process`] call.
    pub fn trigger(&mut self) {
        self.envelope = (1.5 + 0.5 * (1.0 - self.decay)) * (0.3 + 0.7 * self.accent);
    }

    /// Generates the next sample. `trigger` strikes the hi-hat on this call.
    pub fn process(&mut self, trigger: bool) -> f32 {
        if trigger {
            self.trigger();
        }

        let envelope_decay = 1.0 - 0.003 * semitones_to_ratio(-self.decay * 84.0);
        let cut_decay = 1.0 - 0.0025 * semitones_to_ratio(-self.decay * 36.0);

        let mut out = self.metallic_noise.process(2.0 * self.f0);

        let cutoff = (150.0 / self.sample_rate_hz * semitones_to_ratio(self.tone * 72.0))
            .clamp(0.0, 16_000.0 / self.sample_rate_hz);

        self.noise_coloration
            .set_cutoff(cutoff * self.sample_rate_hz);
        self.noise_coloration.set_resonance(3.0 + 6.0 * self.tone);
        self.noise_coloration.process(out);
        out = self.noise_coloration.band();

        let noise_f = (self.f0 * (16.0 + 16.0 * (1.0 - self.noisiness))).clamp(0.0, 0.5);
        self.noise_clock += noise_f;
        if self.noise_clock >= 1.0 {
            self.noise_clock -= 1.0;
            self.noise_sample = self.noise.next_unipolar() - 0.5;
        }
        out += self.noisiness * (self.noise_sample - out);

        let sustain_gain = self.accent * self.decay;
        self.envelope *= if self.envelope > 0.5 {
            envelope_decay
        } else {
            cut_decay
        };
        out *= if self.sustain {
            sustain_gain
        } else {
            self.envelope
        };

        self.hpf.set_cutoff(cutoff * self.sample_rate_hz);
        self.hpf.set_resonance(0.5);
        self.hpf.process(out);
        self.hpf.high()
    }
}
