#![no_std]

//! Duty-modulated PWM audio for embedded targets: effect banks, tiered DSP, mixing.
//!
//! # Quick start
//!
//! ```ignore
//! use embedded_audio::prelude::*;
//!
//! let bank = SoundBank::parse(FLASH_BLOB)?;
//! let mut engine = AudioEngine::new(AudioConfig::default_duty());
//! engine.set_bank(bank);
//! engine.play(1, AdsrSpec::click())?;
//!
//! // In a timer ISR at `config.sample_rate_hz`:
//! let duty = engine.tick();
//! ```
//!
//! # Tiers
//!
//! | Tier | Source | Use |
//! |------|--------|-----|
//! | A | Tone / wavetable / FM synth | UI beeps, alarms |
//! | B | PCM8 / IMA ADPCM in flash | Sampled SFX |
//! | C | Packed ΣΔ bitstream | Premium short sounds |
//!
//! Build banks on the host with the `eaf-bake` tool (`std` feature).

#[cfg(feature = "std")]
extern crate std;

pub mod bank;
pub mod config;
pub mod decode;
pub mod engine;
pub mod envelope;
pub mod error;
pub mod fixed;
pub mod hal;
pub mod output;
pub mod prelude;
pub mod source;
pub mod stream;
pub mod synth;
pub mod tier;
pub mod voice;

#[cfg(feature = "std")]
pub mod encode;

#[cfg(feature = "std")]
pub mod preview;

#[cfg(feature = "fm")]
pub mod profile;

#[cfg(feature = "dsp")]
pub mod dsp;

pub use bank::{BANK_BUILD_CAP, BANK_MAGIC, BANK_VERSION, BankBuilder, EffectEntry, SoundBank};
pub use config::{
    AudioConfig, DEFAULT_PWM_CARRIER_HZ, DEFAULT_PWM_PERIOD, DEFAULT_SAMPLE_RATE_HZ,
    crossfade_step_q8,
};
pub use decode::{AdpcmDecoder, AdpcmStream, Pcm8Stream};
pub use engine::{AudioEngine, VoiceStealingPolicy};
pub use envelope::{Adsr, AdsrSpec};
pub use error::AudioError;
pub use fixed::{db_to_q8, q8_to_db};
pub use hal::{
    DmaDoubleBuffer, DutyBuffer, PwmDutySink, fill_buffer_into, fill_dma_half_buffers, tick_into,
};
pub use output::{
    DutyMode, PwmMapper, SigmaDelta, SigmaDelta2ndOrder, pcm_to_dac_u8, pcm_to_dac_u12,
    pcm_to_dac_u16, pcm_to_i16, pcm_to_i32,
};
pub use source::VoiceSource;
pub use stream::SigmaDeltaBitStream;
pub use synth::{
    FmVoice, PULSE_25_TABLE, SAW_TABLE, SINE_TABLE, SQUARE_TABLE, TRIANGLE_TABLE, ToneParams,
    ToneVoice, Waveform, WavetableVoice, generate_wavetable_fixed,
};
pub use tier::{EffectKind, flags};

#[cfg(feature = "std")]
pub use preview::{render_effect_pcm, render_effect_wav};

#[cfg(feature = "fm")]
pub use output::{FmMapper, FmTick};

#[cfg(feature = "fm")]
pub use profile::markham;

#[cfg(feature = "dsp")]
pub use dsp::{
    AudioLmsFilter, AudioMeter, AudioSpectrumAnalyzer, AudioStats, BiquadAudioFilter,
    BiquadAudioFilterQ15, EnvelopeFollower, GoertzelDetector, WindowType,
};
