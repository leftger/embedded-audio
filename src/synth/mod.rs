pub mod fm;
pub mod tone;
pub mod wavetable;

pub use fm::FmVoice;
pub use tone::{ToneParams, ToneVoice, Waveform};
pub use wavetable::{
    PULSE_25_TABLE, SAW_TABLE, SINE_TABLE, SQUARE_TABLE, TRIANGLE_TABLE, WavetableVoice,
    generate_wavetable_fixed,
};
