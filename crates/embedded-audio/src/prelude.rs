pub use crate::bank::{BANK_MAGIC, BANK_VERSION, BankBuilder, EffectEntry, SoundBank};
pub use crate::config::{
    AudioConfig, DEFAULT_PWM_PERIOD, DEFAULT_SAMPLE_RATE_HZ, crossfade_step_q8,
};
pub use crate::engine::{AudioEngine, VoiceStealingPolicy};
pub use crate::envelope::{Adsr, AdsrSpec};
pub use crate::error::AudioError;
pub use crate::fixed::{db_to_q8, pan_to_gains_q8, q8_to_db, soft_limit_i8, soft_limit_i16};
pub use crate::fx::{AntiPopRamp, DelayLine, Overdrive, RampState, Tremolo, Wavefolder};
pub use crate::hal::{
    AudioProcessor, DmaDoubleBuffer, DutyBuffer, InPlaceAudioProcessor, PwmDutySink,
    fill_buffer_into, fill_dma_half_buffers, tick_into,
};
pub use crate::output::{
    DutyMode, PwmMapper, SigmaDelta, SigmaDelta2ndOrder, pcm_to_dac_u8, pcm_to_dac_u12,
    pcm_to_dac_u16, pcm_to_i16, pcm_to_i32,
};
pub use crate::pluck::KarplusPluck;
pub use crate::synth::{
    PULSE_25_TABLE, PolyBlepOscillator, PolyBlepVoice, PolyBlepWaveform, SAW_TABLE, SINE_TABLE,
    SQUARE_TABLE, TRIANGLE_TABLE, Waveform, WavetableVoice, generate_wavetable_fixed,
};
pub use crate::tier::{EffectKind, flags};

#[cfg(feature = "dsp")]
pub use crate::dsp::{
    AudioLmsFilter, AudioMeter, AudioSpectrumAnalyzer, AudioStats, BiquadAudioFilter,
    BiquadAudioFilterQ15, CicInterpolator, DynamicCompressor, DynamicsCompressor, EnvelopeFollower,
    GoertzelDetector, GoertzelDetectorQ15, NoiseGate, PdmDecimator, PeakEnvelopeFollower,
    PeakEnvelopeFollowerQ15, RmsEnvelopeFollower, RmsEnvelopeFollowerQ15, VadDetectorQ15,
    WindowType,
};

#[cfg(feature = "dsp")]
pub use crate::drums::{AnalogBassDrum, AnalogSnareDrum, HiHat};
