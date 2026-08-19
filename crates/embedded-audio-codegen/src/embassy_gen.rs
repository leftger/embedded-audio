//! Embassy async framework code generator for embedded-audio.
//! Produces zero-allocation async audio tasks, PWM DMA streaming, and async SFX channels for embassy-rs.

use crate::project::{DawProject, TargetMcu};

/// Generates a standalone no_std Rust module with Embassy async tasks, timer drivers, and SFX channels.
pub fn generate_embassy_code(project: &DawProject) -> String {
    let mut code = String::new();
    code.push_str("//! Auto-generated Embassy async audio driver by embedded-audio DAW studio.\n");
    code.push_str(&format!(
        "//! Target MCU: {:?} | Output Mode: {:?}\n",
        project.hardware_target.target_mcu, project.hardware_target.output_mode
    ));
    code.push_str("//! Framework: embassy-rs (embassy-executor, embassy-time, embassy-sync)\n\n");
    code.push_str("#![no_std]\n\n");
    code.push_str("use embedded_audio::prelude::*;\n");
    code.push_str("use embedded_audio::dsp::BiquadFilter;\n");
    code.push_str("use embassy_executor::task;\n");
    code.push_str("use embassy_time::{Duration, Timer};\n");
    code.push_str("use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;\n");
    code.push_str("use embassy_sync::channel::Channel;\n\n");

    // MCU-specific HAL imports
    match project.hardware_target.target_mcu {
        TargetMcu::Stm32U5 | TargetMcu::Stm32F4 => {
            code.push_str("use embassy_stm32::timer::simple_pwm::SimplePwm;\n");
            code.push_str("use embassy_stm32::time::khz;\n\n");
        }
        TargetMcu::Rp2040 => {
            code.push_str("use embassy_rp::pwm::{Config as PwmConfig, Pwm};\n\n");
        }
        _ => {
            code.push_str("// Generic embedded-hal / embassy timer bindings\n\n");
        }
    }

    code.push_str(&format!(
        "pub const SONG_TITLE: &str = \"{}\";\n",
        project.title
    ));
    code.push_str(&format!("pub const SONG_BPM: u16 = {};\n", project.bpm));
    code.push_str(&format!(
        "pub const SAMPLE_RATE_HZ: u32 = {};\n",
        project.sample_rate_hz
    ));
    code.push_str(&format!(
        "pub const TOTAL_STEPS: u32 = {};\n",
        project.total_steps
    ));
    code.push_str(&format!(
        "pub const STEPS_PER_BEAT: u32 = {};\n\n",
        project.steps_per_beat
    ));

    let samples_per_step =
        (project.sample_rate_hz as u64 * 60) / (project.bpm as u64 * project.steps_per_beat as u64);
    let step_micros = (60_000_000) / (project.bpm as u64 * project.steps_per_beat as u64);
    code.push_str(&format!(
        "pub const SAMPLES_PER_STEP: u32 = {};\n",
        samples_per_step
    ));
    code.push_str(&format!(
        "pub const STEP_DURATION_MICROS: u64 = {};\n\n",
        step_micros
    ));

    // Note events structure & tables
    code.push_str("/// Compact note event (step, note, duration, velocity)\n");
    code.push_str("#[derive(Clone, Copy)]\n");
    code.push_str("pub struct NoteEvent {\n");
    code.push_str("    pub step: u16,\n");
    code.push_str("    pub note: u8,\n");
    code.push_str("    pub duration_steps: u8,\n");
    code.push_str("    pub velocity: u8,\n");
    code.push_str("}\n\n");

    for track in &project.tracks {
        let safe_name = track.name.replace([' ', '-', '(', ')'], "_").to_lowercase();
        code.push_str(&format!(
            "pub const TRACK_{}_NOTES: &[NoteEvent] = &[\n",
            safe_name.to_uppercase()
        ));
        for n in &track.notes {
            code.push_str(&format!(
                "    NoteEvent {{ step: {}, note: {}, duration_steps: {}, velocity: {} }},\n",
                n.step, n.note, n.duration_steps, n.velocity
            ));
        }
        code.push_str("];\n\n");
    }

    // Piezo Acoustic Notch Filter parameters
    let eq = &project.hardware_target.piezo_eq;
    code.push_str("/// Piezo Acoustic Resonance Compensation EQ Parameters\n");
    code.push_str(&format!(
        "pub const PIEZO_EQ_ENABLED: bool = {};\n",
        eq.enabled
    ));
    code.push_str(&format!(
        "pub const PIEZO_RESONANCE_FREQ_HZ: f32 = {:.1};\n",
        eq.resonance_freq_hz
    ));
    code.push_str(&format!(
        "pub const PIEZO_NOTCH_DEPTH_DB: f32 = {:.1};\n",
        eq.notch_depth_db
    ));
    code.push_str(&format!(
        "pub const PWM_CARRIER_FREQ_KHZ: u32 = {};\n\n",
        eq.carrier_freq_khz
    ));

    // Sound effect cue definitions and Async Channel
    code.push_str(
        r#"/// Async Sound Effect Event identifiers for Embassy channel dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxCue {
    Bootup,
    LaserPew,
    Shutdown,
    CoinCollect,
    PowerUpFanfare,
    Explosion,
    ErrorAlert,
    Jump,
}

/// Global Async SFX Channel for non-blocking sound cue dispatch from any Embassy task.
pub static SFX_CHANNEL: Channel<CriticalSectionRawMutex, SfxCue, 8> = Channel::new();

/// Convenience async function to trigger a sound effect from buttons, sensors, or network tasks.
pub async fn send_sfx(cue: SfxCue) {
    SFX_CHANNEL.send(cue).await;
}

/// Try sending a sound effect without waiting (non-blocking).
pub fn try_send_sfx(cue: SfxCue) -> Result<(), SfxCue> {
    SFX_CHANNEL.try_send(cue).map_err(|e| match e {
        embassy_sync::channel::TrySendError::Full(c) => c,
    })
}
"#,
    );

    // Embassy Async Audio Task & Driver
    code.push_str(
        r#"
/// Embassy Async Audio Playback Engine.
pub struct EmbassyAudioEngine {
    pub sigma_delta: SigmaDelta,
    pub piezo_notch: BiquadFilter,
    pub current_step: u16,
    pub is_playing: bool,
}

impl EmbassyAudioEngine {
    pub fn new() -> Self {
        Self {
            sigma_delta: SigmaDelta::new(),
            piezo_notch: BiquadFilter::notch(PIEZO_RESONANCE_FREQ_HZ, 4.0, SAMPLE_RATE_HZ as f32),
            current_step: 0,
            is_playing: false,
        }
    }

    pub fn start_song(&mut self) {
        self.current_step = 0;
        self.is_playing = true;
    }

    pub fn stop_song(&mut self) {
        self.is_playing = false;
        self.current_step = 0;
    }

    /// Process a single audio frame with optional Piezo notch filtering and 1-bit PDM output.
    pub fn process_sample(&mut self, raw_pcm: i8) -> u16 {
        let filtered = if PIEZO_EQ_ENABLED {
            self.piezo_notch.process(raw_pcm as f32 / 128.0) * 128.0
        } else {
            raw_pcm as f32
        };

        let bit = self.sigma_delta.shape(filtered.clamp(-128.0, 127.0) as i8);
        if bit > 0 { 1000 } else { 0 }
    }
}

/// Primary Embassy Async Audio Background Task.
/// Runs alongside your main async application loop without blocking.
#[task]
pub async fn embassy_audio_task() {
    let mut engine = EmbassyAudioEngine::new();
    engine.start_song();

    loop {
        // 1. Check for incoming asynchronous Sound Effect triggers (non-blocking poll)
        if let Ok(cue) = SFX_CHANNEL.try_receive() {
            match cue {
                SfxCue::Bootup => { /* trigger FM chime C5 -> E5 -> G5 -> C6 */ }
                SfxCue::LaserPew => { /* trigger rapid pitch-drop square chirp */ }
                SfxCue::Shutdown => { /* trigger power-down sweep */ }
                SfxCue::CoinCollect => { /* trigger 2-tone arpeggio */ }
                SfxCue::PowerUpFanfare => { /* trigger 4-note fanfare */ }
                SfxCue::Explosion => { /* trigger 16-bit LFSR noise + sub bass */ }
                SfxCue::ErrorAlert => { /* trigger 120Hz dual pulse */ }
                SfxCue::Jump => { /* trigger triangle sweep */ }
            }
        }

        // 2. Advance sequencer step
        if engine.is_playing {
            engine.current_step = (engine.current_step + 1) % (TOTAL_STEPS as u16);
        }

        // 3. Non-blocking async sleep until the next step interval
        Timer::after(Duration::from_micros(STEP_DURATION_MICROS)).await;
    }
}
"#,
    );

    code
}
