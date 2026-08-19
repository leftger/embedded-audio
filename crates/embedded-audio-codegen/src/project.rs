//! DAW Project data structures and curated chiptune/embedded audio presets & sound effects.

use serde::{Deserialize, Serialize};

/// Supported waveform choices for synth instruments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaveformType {
    Sine,
    Square,
    Pulse25,
    Pulse12_5,
    Sawtooth,
    Triangle,
    Noise,
}

/// Synthesis type for an instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstrumentKind {
    Tone {
        waveform: WaveformType,
        duty: u8,
    },
    Fm {
        mod_ratio_x100: u16,
        mod_index_x100: u16,
        feedback_x100: u16,
    },
    Wavetable {
        preset: u8,
        custom_samples: Option<Vec<i8>>,
    },
    Sample {
        name: String,
        is_adpcm: bool,
        raw_pcm: Vec<i8>,
        sample_rate_hz: u32,
    },
    Noise,
}

/// ADSR envelope parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsrConfig {
    pub attack_ms: u16,
    pub decay_ms: u16,
    pub sustain_q8: u8,
    pub release_ms: u16,
}

impl Default for AdsrConfig {
    fn default() -> Self {
        Self {
            attack_ms: 10,
            decay_ms: 50,
            sustain_q8: 200,
            release_ms: 80,
        }
    }
}

/// A playable instrument/patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub id: usize,
    pub name: String,
    pub kind: InstrumentKind,
    pub adsr: AdsrConfig,
    pub volume_q8: u8,
    pub pan_q8: i8,
}

/// A musical note event in the piano-roll / step sequencer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteEvent {
    pub step: u32,
    pub note: u8, // MIDI note 0..127 (e.g. 60 = C4)
    pub duration_steps: u32,
    pub velocity: u8, // 0..127
}

/// A track in the DAW.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: usize,
    pub name: String,
    pub instrument_id: usize,
    pub muted: bool,
    pub solo: bool,
    pub volume_q8: u8,
    pub notes: Vec<NoteEvent>,
}

/// Microcontroller hardware profile choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetMcu {
    Stm32U5,         // Cortex-M33 @ 160MHz (GPDMA + 32-bit TIM2/TIM5)
    Stm32F4,         // Cortex-M4 @ 168MHz (DMA1/2 + TIM1..TIM8)
    Rp2040,          // Dual Cortex-M0+ @ 125MHz (PIO + PWM)
    Esp32RiscV,      // ESP32-C3 / C6 / Generic RISC-V
    GenericCortexM0, // Base Cortex-M0/M0+ 48MHz
}

/// Output modulation scheme for the transducer/speaker pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinOutputMode {
    UltrasonicPwmSigmaDelta, // 250kHz - 1MHz PWM 1-Bit Sigma-Delta (Polyphony, FM & Speech)
    PushPullComplementaryPwm, // TIMx_CH1 + TIMx_CH1N (6.6V P-P Voltage Swing Boost)
    FastArpeggioBeeper,      // 50-120Hz Fast Multiplexing (Tim Follin 1-bit Tracker)
    DacOrI2sDirect,          // Internal 12-bit DAC or External I2S DAC
}

/// Piezo acoustic resonance compensation equalizer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PiezoAcousticConfig {
    pub enabled: bool,
    pub resonance_freq_hz: f32, // Typical piezo peak 2000Hz - 4500Hz
    pub notch_depth_db: f32,    // Tame resonance spike (e.g. -6dB to -18dB)
    pub bass_boost_db: f32,     // Sub-harmonic compensation
    pub carrier_freq_khz: u32,  // Ultrasonic PWM carrier (e.g. 500 kHz)
}

impl Default for PiezoAcousticConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            resonance_freq_hz: 3200.0,
            notch_depth_db: -12.0,
            bass_boost_db: 4.0,
            carrier_freq_khz: 500,
        }
    }
}

/// Full Hardware Target & Transducer Configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HardwareTargetConfig {
    pub target_mcu: TargetMcu,
    pub output_mode: PinOutputMode,
    pub timer_bits: u8, // 16 or 32
    pub dma_buffer_size: usize,
    pub piezo_eq: PiezoAcousticConfig,
}

impl Default for HardwareTargetConfig {
    fn default() -> Self {
        Self {
            target_mcu: TargetMcu::Stm32U5,
            output_mode: PinOutputMode::UltrasonicPwmSigmaDelta,
            timer_bits: 32,
            dma_buffer_size: 256,
            piezo_eq: PiezoAcousticConfig::default(),
        }
    }
}

/// A complete song or sound cue project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DawProject {
    pub title: String,
    pub author: String,
    pub bpm: u16,
    pub sample_rate_hz: u32,
    pub total_steps: u32,
    pub steps_per_beat: u32,
    pub hardware_target: HardwareTargetConfig,
    pub instruments: Vec<Instrument>,
    pub tracks: Vec<Track>,
}

impl Default for DawProject {
    fn default() -> Self {
        Self::chiptune_odyssey()
    }
}

impl DawProject {
    /// Preset 1: Retro 8-bit platformer anthem.
    pub fn chiptune_odyssey() -> Self {
        let instruments = vec![
            Instrument {
                id: 0,
                name: "Square Lead".to_string(),
                kind: InstrumentKind::Tone {
                    waveform: WaveformType::Square,
                    duty: 128,
                },
                adsr: AdsrConfig {
                    attack_ms: 5,
                    decay_ms: 40,
                    sustain_q8: 220,
                    release_ms: 60,
                },
                volume_q8: 240,
                pan_q8: 0,
            },
            Instrument {
                id: 1,
                name: "Pulse Bass".to_string(),
                kind: InstrumentKind::Tone {
                    waveform: WaveformType::Pulse25,
                    duty: 64,
                },
                adsr: AdsrConfig {
                    attack_ms: 10,
                    decay_ms: 80,
                    sustain_q8: 180,
                    release_ms: 40,
                },
                volume_q8: 255,
                pan_q8: 0,
            },
            Instrument {
                id: 2,
                name: "FM Bell".to_string(),
                kind: InstrumentKind::Fm {
                    mod_ratio_x100: 200,
                    mod_index_x100: 150,
                    feedback_x100: 20,
                },
                adsr: AdsrConfig {
                    attack_ms: 2,
                    decay_ms: 200,
                    sustain_q8: 60,
                    release_ms: 250,
                },
                volume_q8: 200,
                pan_q8: 0,
            },
            Instrument {
                id: 3,
                name: "Noise Hi-Hat".to_string(),
                kind: InstrumentKind::Noise,
                adsr: AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 30,
                    sustain_q8: 0,
                    release_ms: 20,
                },
                volume_q8: 180,
                pan_q8: 0,
            },
        ];

        let tracks = vec![
            Track {
                id: 0,
                name: "Melody (Square Lead)".to_string(),
                instrument_id: 0,
                muted: false,
                solo: false,
                volume_q8: 240,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 60,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 2,
                        note: 64,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 4,
                        note: 67,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 6,
                        note: 72,
                        duration_steps: 4,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 10,
                        note: 67,
                        duration_steps: 2,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 12,
                        note: 64,
                        duration_steps: 2,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 14,
                        note: 60,
                        duration_steps: 2,
                        velocity: 95,
                    },
                    NoteEvent {
                        step: 16,
                        note: 65,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 18,
                        note: 69,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 20,
                        note: 72,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 22,
                        note: 77,
                        duration_steps: 4,
                        velocity: 115,
                    },
                    NoteEvent {
                        step: 26,
                        note: 72,
                        duration_steps: 2,
                        velocity: 95,
                    },
                    NoteEvent {
                        step: 28,
                        note: 69,
                        duration_steps: 2,
                        velocity: 95,
                    },
                    NoteEvent {
                        step: 30,
                        note: 65,
                        duration_steps: 2,
                        velocity: 95,
                    },
                ],
            },
            Track {
                id: 1,
                name: "Bass (Pulse 25%)".to_string(),
                instrument_id: 1,
                muted: false,
                solo: false,
                volume_q8: 255,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 36,
                        duration_steps: 4,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 4,
                        note: 36,
                        duration_steps: 4,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 8,
                        note: 41,
                        duration_steps: 4,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 12,
                        note: 43,
                        duration_steps: 4,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 16,
                        note: 41,
                        duration_steps: 4,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 20,
                        note: 41,
                        duration_steps: 4,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 24,
                        note: 36,
                        duration_steps: 4,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 28,
                        note: 43,
                        duration_steps: 4,
                        velocity: 110,
                    },
                ],
            },
            Track {
                id: 2,
                name: "Percussion (LFSR)".to_string(),
                instrument_id: 3,
                muted: false,
                solo: false,
                volume_q8: 190,
                notes: vec![
                    // Beat 1: Kick + Hi-Hat
                    NoteEvent {
                        step: 0,
                        note: 36,
                        duration_steps: 1,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 2,
                        note: 84,
                        duration_steps: 1,
                        velocity: 85,
                    },
                    // Beat 2: Snare + Hi-Hat
                    NoteEvent {
                        step: 4,
                        note: 60,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 6,
                        note: 84,
                        duration_steps: 1,
                        velocity: 85,
                    },
                    // Beat 3: Kick + Hi-Hat
                    NoteEvent {
                        step: 8,
                        note: 36,
                        duration_steps: 1,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 10,
                        note: 84,
                        duration_steps: 1,
                        velocity: 85,
                    },
                    // Beat 4: Snare + Open Hi-Hat
                    NoteEvent {
                        step: 12,
                        note: 60,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 14,
                        note: 90,
                        duration_steps: 1,
                        velocity: 95,
                    },
                    // Second Bar
                    NoteEvent {
                        step: 16,
                        note: 36,
                        duration_steps: 1,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 18,
                        note: 84,
                        duration_steps: 1,
                        velocity: 85,
                    },
                    NoteEvent {
                        step: 20,
                        note: 60,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 22,
                        note: 84,
                        duration_steps: 1,
                        velocity: 85,
                    },
                    NoteEvent {
                        step: 24,
                        note: 36,
                        duration_steps: 1,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 26,
                        note: 84,
                        duration_steps: 1,
                        velocity: 85,
                    },
                    NoteEvent {
                        step: 28,
                        note: 60,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 30,
                        note: 90,
                        duration_steps: 1,
                        velocity: 100,
                    },
                ],
            },
        ];

        Self {
            title: "Chiptune Odyssey".to_string(),
            author: "Retro Hero".to_string(),
            bpm: 125,
            sample_rate_hz: 16000,
            total_steps: 32,
            steps_per_beat: 4,
            hardware_target: HardwareTargetConfig::default(),
            instruments,
            tracks,
        }
    }

    /// Preset 2: FM Cyberpunk Dystopia.
    pub fn fm_cyberpunk() -> Self {
        let instruments = vec![
            Instrument {
                id: 0,
                name: "Cyber FM Bell".to_string(),
                kind: InstrumentKind::Fm {
                    mod_ratio_x100: 350,
                    mod_index_x100: 220,
                    feedback_x100: 40,
                },
                adsr: AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 180,
                    sustain_q8: 80,
                    release_ms: 200,
                },
                volume_q8: 230,
                pan_q8: 0,
            },
            Instrument {
                id: 1,
                name: "Saw Acid Bass".to_string(),
                kind: InstrumentKind::Tone {
                    waveform: WaveformType::Sawtooth,
                    duty: 128,
                },
                adsr: AdsrConfig {
                    attack_ms: 4,
                    decay_ms: 90,
                    sustain_q8: 150,
                    release_ms: 50,
                },
                volume_q8: 255,
                pan_q8: 0,
            },
            Instrument {
                id: 2,
                name: "Techno Noise".to_string(),
                kind: InstrumentKind::Noise,
                adsr: AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 45,
                    sustain_q8: 0,
                    release_ms: 30,
                },
                volume_q8: 200,
                pan_q8: 0,
            },
        ];

        let tracks = vec![
            Track {
                id: 0,
                name: "FM Arp Lead".to_string(),
                instrument_id: 0,
                muted: false,
                solo: false,
                volume_q8: 230,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 72,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 1,
                        note: 75,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 2,
                        note: 79,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 3,
                        note: 82,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 4,
                        note: 72,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 5,
                        note: 75,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 6,
                        note: 79,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 7,
                        note: 84,
                        duration_steps: 1,
                        velocity: 115,
                    },
                    NoteEvent {
                        step: 8,
                        note: 70,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 9,
                        note: 74,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 10,
                        note: 77,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 11,
                        note: 81,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 12,
                        note: 70,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 13,
                        note: 74,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 14,
                        note: 77,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 15,
                        note: 82,
                        duration_steps: 1,
                        velocity: 115,
                    },
                ],
            },
            Track {
                id: 1,
                name: "Acid Bassline".to_string(),
                instrument_id: 1,
                muted: false,
                solo: false,
                volume_q8: 255,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 36,
                        duration_steps: 2,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 2,
                        note: 36,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 4,
                        note: 48,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 6,
                        note: 36,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 8,
                        note: 34,
                        duration_steps: 2,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 10,
                        note: 34,
                        duration_steps: 2,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 12,
                        note: 46,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 14,
                        note: 34,
                        duration_steps: 2,
                        velocity: 100,
                    },
                ],
            },
            Track {
                id: 2,
                name: "Hi-Hat & Clack".to_string(),
                instrument_id: 2,
                muted: false,
                solo: false,
                volume_q8: 200,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 60,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 2,
                        note: 60,
                        duration_steps: 1,
                        velocity: 80,
                    },
                    NoteEvent {
                        step: 4,
                        note: 60,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 6,
                        note: 60,
                        duration_steps: 1,
                        velocity: 80,
                    },
                    NoteEvent {
                        step: 8,
                        note: 60,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 10,
                        note: 60,
                        duration_steps: 1,
                        velocity: 80,
                    },
                    NoteEvent {
                        step: 12,
                        note: 60,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 14,
                        note: 60,
                        duration_steps: 1,
                        velocity: 80,
                    },
                ],
            },
        ];

        Self {
            title: "Cyberpunk Dystopia".to_string(),
            author: "FM Syndicate".to_string(),
            bpm: 138,
            sample_rate_hz: 16000,
            total_steps: 16,
            steps_per_beat: 4,
            hardware_target: HardwareTargetConfig::default(),
            instruments,
            tracks,
        }
    }

    /// Preset 3: Fast-paced 160 BPM Boss Battle theme.
    pub fn boss_battle() -> Self {
        let instruments = vec![
            Instrument {
                id: 0,
                name: "Staccato Square".to_string(),
                kind: InstrumentKind::Tone {
                    waveform: WaveformType::Square,
                    duty: 128,
                },
                adsr: AdsrConfig {
                    attack_ms: 2,
                    decay_ms: 30,
                    sustain_q8: 120,
                    release_ms: 25,
                },
                volume_q8: 245,
                pan_q8: 0,
            },
            Instrument {
                id: 1,
                name: "Battle Triangle Bass".to_string(),
                kind: InstrumentKind::Tone {
                    waveform: WaveformType::Triangle,
                    duty: 128,
                },
                adsr: AdsrConfig {
                    attack_ms: 5,
                    decay_ms: 60,
                    sustain_q8: 200,
                    release_ms: 40,
                },
                volume_q8: 255,
                pan_q8: 0,
            },
            Instrument {
                id: 2,
                name: "Snare & Explosion".to_string(),
                kind: InstrumentKind::Noise,
                adsr: AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 70,
                    sustain_q8: 0,
                    release_ms: 40,
                },
                volume_q8: 210,
                pan_q8: 0,
            },
        ];

        let tracks = vec![
            Track {
                id: 0,
                name: "Speed Lead".to_string(),
                instrument_id: 0,
                muted: false,
                solo: false,
                volume_q8: 245,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 64,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 1,
                        note: 64,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 2,
                        note: 71,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 3,
                        note: 72,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 4,
                        note: 76,
                        duration_steps: 2,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 6,
                        note: 72,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 7,
                        note: 71,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 8,
                        note: 69,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 10,
                        note: 67,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 12,
                        note: 64,
                        duration_steps: 4,
                        velocity: 125,
                    },
                ],
            },
            Track {
                id: 1,
                name: "Fast Bass".to_string(),
                instrument_id: 1,
                muted: false,
                solo: false,
                volume_q8: 255,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 40,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 2,
                        note: 40,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 4,
                        note: 43,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 6,
                        note: 45,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 8,
                        note: 47,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 10,
                        note: 45,
                        duration_steps: 2,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 12,
                        note: 40,
                        duration_steps: 4,
                        velocity: 120,
                    },
                ],
            },
            Track {
                id: 2,
                name: "Drums".to_string(),
                instrument_id: 2,
                muted: false,
                solo: false,
                volume_q8: 210,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 60,
                        duration_steps: 1,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 2,
                        note: 60,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 4,
                        note: 60,
                        duration_steps: 2,
                        velocity: 125,
                    },
                    NoteEvent {
                        step: 6,
                        note: 60,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 8,
                        note: 60,
                        duration_steps: 1,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 10,
                        note: 60,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 12,
                        note: 60,
                        duration_steps: 2,
                        velocity: 125,
                    },
                    NoteEvent {
                        step: 14,
                        note: 60,
                        duration_steps: 1,
                        velocity: 100,
                    },
                ],
            },
        ];

        Self {
            title: "Boss Battle 160BPM".to_string(),
            author: "8-Bit Warrior".to_string(),
            bpm: 160,
            sample_rate_hz: 16000,
            total_steps: 16,
            steps_per_beat: 4,
            hardware_target: HardwareTargetConfig::default(),
            instruments,
            tracks,
        }
    }

    /// Preset 4: Mellow Lo-Fi Nostalgia.
    pub fn lofi_nostalgia() -> Self {
        let instruments = vec![
            Instrument {
                id: 0,
                name: "Warm Sine Wavetable".to_string(),
                kind: InstrumentKind::Wavetable {
                    preset: 0,
                    custom_samples: None,
                },
                adsr: AdsrConfig {
                    attack_ms: 25,
                    decay_ms: 120,
                    sustain_q8: 180,
                    release_ms: 150,
                },
                volume_q8: 240,
                pan_q8: 0,
            },
            Instrument {
                id: 1,
                name: "Mellow Triangle".to_string(),
                kind: InstrumentKind::Tone {
                    waveform: WaveformType::Triangle,
                    duty: 128,
                },
                adsr: AdsrConfig {
                    attack_ms: 20,
                    decay_ms: 150,
                    sustain_q8: 200,
                    release_ms: 100,
                },
                volume_q8: 250,
                pan_q8: 0,
            },
        ];

        let tracks = vec![
            Track {
                id: 0,
                name: "Warm Melody".to_string(),
                instrument_id: 0,
                muted: false,
                solo: false,
                volume_q8: 240,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 65,
                        duration_steps: 4,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 4,
                        note: 69,
                        duration_steps: 4,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 8,
                        note: 72,
                        duration_steps: 4,
                        velocity: 105,
                    },
                    NoteEvent {
                        step: 12,
                        note: 69,
                        duration_steps: 4,
                        velocity: 95,
                    },
                ],
            },
            Track {
                id: 1,
                name: "Deep Triangle".to_string(),
                instrument_id: 1,
                muted: false,
                solo: false,
                volume_q8: 250,
                notes: vec![
                    NoteEvent {
                        step: 0,
                        note: 41,
                        duration_steps: 8,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 8,
                        note: 45,
                        duration_steps: 8,
                        velocity: 110,
                    },
                ],
            },
        ];

        Self {
            title: "Lo-Fi Nostalgia".to_string(),
            author: "Chill MCU".to_string(),
            bpm: 82,
            sample_rate_hz: 16000,
            total_steps: 16,
            steps_per_beat: 4,
            hardware_target: HardwareTargetConfig::default(),
            instruments,
            tracks,
        }
    }

    /// Preset 5: SFX Showcase (Bootup, Lasers, Coin, Power-up, Alert, Explosion, Shutdown).
    pub fn sfx_showcase() -> Self {
        let instruments = vec![
            Instrument {
                id: 0,
                name: "Laser / Beep Synth".to_string(),
                kind: InstrumentKind::Tone {
                    waveform: WaveformType::Square,
                    duty: 128,
                },
                adsr: AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 25,
                    sustain_q8: 40,
                    release_ms: 20,
                },
                volume_q8: 255,
                pan_q8: 0,
            },
            Instrument {
                id: 1,
                name: "Chime & Fanfare".to_string(),
                kind: InstrumentKind::Fm {
                    mod_ratio_x100: 200,
                    mod_index_x100: 160,
                    feedback_x100: 10,
                },
                adsr: AdsrConfig {
                    attack_ms: 2,
                    decay_ms: 150,
                    sustain_q8: 100,
                    release_ms: 200,
                },
                volume_q8: 240,
                pan_q8: 0,
            },
            Instrument {
                id: 2,
                name: "Impact / Noise Burst".to_string(),
                kind: InstrumentKind::Noise,
                adsr: AdsrConfig {
                    attack_ms: 1,
                    decay_ms: 120,
                    sustain_q8: 0,
                    release_ms: 60,
                },
                volume_q8: 240,
                pan_q8: 0,
            },
            Instrument {
                id: 3,
                name: "Sub Bass Thud".to_string(),
                kind: InstrumentKind::Tone {
                    waveform: WaveformType::Triangle,
                    duty: 128,
                },
                adsr: AdsrConfig {
                    attack_ms: 2,
                    decay_ms: 160,
                    sustain_q8: 0,
                    release_ms: 80,
                },
                volume_q8: 255,
                pan_q8: 0,
            },
        ];

        let tracks = vec![
            Track {
                id: 0,
                name: "Laser & Blasts".to_string(),
                instrument_id: 0,
                muted: false,
                solo: false,
                volume_q8: 255,
                notes: vec![
                    // [Step 4..6] Laser Pew-Pew (Rapid high-pitch pitch fall)
                    NoteEvent {
                        step: 4,
                        note: 88,
                        duration_steps: 1,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 5,
                        note: 81,
                        duration_steps: 1,
                        velocity: 105,
                    },
                    NoteEvent {
                        step: 6,
                        note: 74,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    // [Step 8..10] Second Laser Burst
                    NoteEvent {
                        step: 8,
                        note: 86,
                        duration_steps: 1,
                        velocity: 120,
                    },
                    NoteEvent {
                        step: 9,
                        note: 79,
                        duration_steps: 1,
                        velocity: 105,
                    },
                    // [Step 12] Coin Pickup (B5 -> E6)
                    NoteEvent {
                        step: 12,
                        note: 83,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 13,
                        note: 88,
                        duration_steps: 2,
                        velocity: 125,
                    },
                    // [Step 20..22] Warning Alarm Beeps
                    NoteEvent {
                        step: 20,
                        note: 55,
                        duration_steps: 1,
                        velocity: 127,
                    },
                    NoteEvent {
                        step: 22,
                        note: 55,
                        duration_steps: 1,
                        velocity: 127,
                    },
                    // [Step 28..31] System Shutdown Power-Down Slide
                    NoteEvent {
                        step: 28,
                        note: 67,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 29,
                        note: 62,
                        duration_steps: 1,
                        velocity: 90,
                    },
                    NoteEvent {
                        step: 30,
                        note: 55,
                        duration_steps: 1,
                        velocity: 70,
                    },
                    NoteEvent {
                        step: 31,
                        note: 43,
                        duration_steps: 2,
                        velocity: 50,
                    },
                ],
            },
            Track {
                id: 1,
                name: "Bootup & Fanfare".to_string(),
                instrument_id: 1,
                muted: false,
                solo: false,
                volume_q8: 240,
                notes: vec![
                    // [Step 0..2] System Bootup Chime (C5 -> E5 -> G5 -> C6)
                    NoteEvent {
                        step: 0,
                        note: 72,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 1,
                        note: 76,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 2,
                        note: 79,
                        duration_steps: 1,
                        velocity: 115,
                    },
                    NoteEvent {
                        step: 3,
                        note: 84,
                        duration_steps: 2,
                        velocity: 127,
                    },
                    // [Step 15..18] Power-Up Jingle
                    NoteEvent {
                        step: 15,
                        note: 65,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    NoteEvent {
                        step: 16,
                        note: 69,
                        duration_steps: 1,
                        velocity: 105,
                    },
                    NoteEvent {
                        step: 17,
                        note: 72,
                        duration_steps: 1,
                        velocity: 110,
                    },
                    NoteEvent {
                        step: 18,
                        note: 77,
                        duration_steps: 3,
                        velocity: 125,
                    },
                ],
            },
            Track {
                id: 2,
                name: "Explosions & Noise".to_string(),
                instrument_id: 2,
                muted: false,
                solo: false,
                volume_q8: 240,
                notes: vec![
                    // [Step 4] High laser sizzle burst (Note 90)
                    NoteEvent {
                        step: 4,
                        note: 90,
                        duration_steps: 1,
                        velocity: 100,
                    },
                    // [Step 12] Micro coin sparkle (Note 96)
                    NoteEvent {
                        step: 12,
                        note: 96,
                        duration_steps: 1,
                        velocity: 80,
                    },
                    // [Step 24] Massive Deep Explosion (Low Note 28 = heavy LFSR rumble)
                    NoteEvent {
                        step: 24,
                        note: 28,
                        duration_steps: 4,
                        velocity: 127,
                    },
                ],
            },
            Track {
                id: 3,
                name: "Sub Thud".to_string(),
                instrument_id: 3,
                muted: false,
                solo: false,
                volume_q8: 255,
                notes: vec![
                    // [Step 24] 40Hz Sub Bass Thud layer for explosion
                    NoteEvent {
                        step: 24,
                        note: 28,
                        duration_steps: 4,
                        velocity: 127,
                    },
                ],
            },
        ];

        Self {
            title: "SFX Showcase Pack".to_string(),
            author: "Retro Sound FX".to_string(),
            bpm: 120,
            sample_rate_hz: 16000,
            total_steps: 32,
            steps_per_beat: 4,
            hardware_target: HardwareTargetConfig::default(),
            instruments,
            tracks,
        }
    }
}
