use crate::decode::{AdpcmStream, Pcm8Stream};
use crate::envelope::AdsrSpec;
use crate::stream::SigmaDeltaBitStream;
use crate::synth::{FmVoice, ToneParams, ToneVoice, Waveform, WavetableVoice};
use crate::tier::EffectKind;

/// Active generator for one mixer voice.
#[derive(Debug, Clone, Copy)]
pub enum VoiceSource<'a> {
    Idle,
    Tone(ToneVoice),
    Wavetable(WavetableVoice<'a>),
    Fm(FmVoice),
    Pcm8(Pcm8Stream<'a>),
    Adpcm(AdpcmStream<'a>),
    SigmaDeltaBits(SigmaDeltaBitStream<'a>),
}

impl<'a> VoiceSource<'a> {
    pub const fn idle() -> Self {
        Self::Idle
    }

    pub fn stop(&mut self) {
        *self = Self::Idle;
    }

    pub fn is_active(&self) -> bool {
        match self {
            Self::Idle => false,
            Self::Tone(v) => v.is_active(),
            Self::Wavetable(v) => v.is_active(),
            Self::Fm(v) => v.is_active(),
            Self::Pcm8(v) => !v.is_done(),
            Self::Adpcm(v) => !v.is_done(),
            Self::SigmaDeltaBits(v) => !v.is_done(),
        }
    }

    /// Dominant carrier for FM hardware (Tier A tone only).
    pub fn carrier_hz(&self, sample_rate_hz: u32) -> Option<u32> {
        match self {
            Self::Tone(v) => {
                let hz = v.carrier_hz(sample_rate_hz);
                if hz == 0 { None } else { Some(hz) }
            }
            _ => None,
        }
    }

    pub fn next_raw_sample(&mut self) -> Option<i8> {
        match self {
            Self::Idle => None,
            Self::Tone(v) => v.next_sample(),
            Self::Wavetable(v) => v.next_sample(),
            Self::Fm(v) => v.next_sample(),
            Self::Pcm8(v) => v.next_sample(),
            Self::Adpcm(v) => v.next_sample(),
            Self::SigmaDeltaBits(v) => v.next_sample(),
        }
    }

    pub fn start_tone(
        &mut self,
        freq_hz: u32,
        duration_ms: u16,
        waveform: Waveform,
        sample_rate_hz: u32,
    ) {
        let mut voice = ToneVoice::new();
        voice.start(
            ToneParams {
                freq_hz,
                duration_ms,
                waveform,
                adsr: AdsrSpec::click(),
            },
            sample_rate_hz,
        );
        *self = Self::Tone(voice);
    }

    pub fn start_wavetable(&mut self, table: &'a [u8], freq_hz: u32, sample_rate_hz: u32) {
        let mut voice = WavetableVoice::new(table);
        voice.start(freq_hz, sample_rate_hz);
        *self = Self::Wavetable(voice);
    }

    pub fn start_fm(&mut self, carrier_hz: u32, mod_ratio_cent: u16, sample_rate_hz: u32) {
        let mut voice = FmVoice::new();
        voice.start(carrier_hz, mod_ratio_cent, 72, sample_rate_hz);
        *self = Self::Fm(voice);
    }

    pub fn start_from_entry(
        &mut self,
        kind: EffectKind,
        flags: u8,
        param0: u16,
        param1: u16,
        payload: &'a [u8],
        sample_rate_hz: u32,
    ) -> bool {
        match kind {
            EffectKind::Tone => {
                self.start_tone(param0 as u32, param1, Waveform::Sine, sample_rate_hz);
                true
            }
            EffectKind::Wavetable => {
                if payload.len() < 256 {
                    return false;
                }
                self.start_wavetable(payload, param0 as u32, sample_rate_hz);
                true
            }
            EffectKind::Fm => {
                self.start_fm(param0 as u32, param1, sample_rate_hz);
                true
            }
            EffectKind::Pcm8 => {
                *self = Self::Pcm8(Pcm8Stream::new(payload, flags));
                true
            }
            EffectKind::Adpcm => {
                if let Some(s) = AdpcmStream::new(payload, flags) {
                    *self = Self::Adpcm(s);
                    true
                } else {
                    false
                }
            }
            EffectKind::SigmaDeltaBits => {
                *self = Self::SigmaDeltaBits(SigmaDeltaBitStream::new(payload, flags));
                true
            }
        }
    }
}
