//! Host-side preview helpers (`std` feature).

use std::vec::Vec;

use crate::{AdsrSpec, AudioConfig, AudioEngine, AudioError, SoundBank};

/// Render `effect_id` from a parsed bank into signed PCM samples (pre-PWM bus).
pub fn render_effect_pcm(
    bank: SoundBank<'_>,
    effect_id: u16,
    max_samples: usize,
    adsr: AdsrSpec,
) -> Result<Vec<i8>, AudioError> {
    let mut engine = AudioEngine::new(AudioConfig::default_duty());
    engine.set_bank(bank);
    engine.play(effect_id, adsr)?;

    let cap = max_samples.min(256 * 1024);
    let mut out = Vec::with_capacity(cap);
    while engine.is_playing() && out.len() < cap {
        out.push(engine.tick_pcm());
    }
    Ok(out)
}

#[cfg(feature = "std")]
pub use crate::encode::wav::{pcm_i8_to_u8, write_mono_u8};

/// Render to an 8-bit mono WAV file on disk.
#[cfg(feature = "std")]
pub fn render_effect_wav(
    bank: SoundBank<'_>,
    effect_id: u16,
    path: &str,
    max_samples: usize,
    adsr: AdsrSpec,
) -> Result<(), AudioError> {
    let rate = bank.sample_rate_hz;
    let pcm = render_effect_pcm(bank, effect_id, max_samples, adsr)?;
    let u8s = pcm_i8_to_u8(&pcm);
    write_mono_u8(path, rate, &u8s).map_err(|_| AudioError::PreviewIo)
}
