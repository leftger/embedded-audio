//! # PWM audio over GPDMA on the STM32WBA65RI (NUCLEO-WBA65RI)
//!
//! A "showcase" firmware example that joins [`embedded-audio`] to
//! [`embassy-stm32`]: a fixed-frequency PWM carrier on **TIM3_CH1** is
//! modulated by a continuous stream of duty-cycle values pushed by
//! **GPDMA1** straight into the timer's capture/compare register.
//!
//! ```text
//!  AudioEngine (wavetables + ADSR + sigma-delta)
//!        │  fill_duty_buffer(&mut [u16; CHUNK])
//!        ▼
//!  RingBufferedPwmChannel::write_exact().await   <- this example
//!        │  GPDMA1_CH0, memory -> TIM3->CCR1, on every TIM3 update event
//!        ▼
//!  TIM3_CH1 (PA2) ── PWM ──► RC low-pass ──► piezo / small speaker
//! ```
//!
//! The CPU renders one chunk of audio and then *sleeps* until the DMA has
//! drained enough of the ring buffer for the next chunk, so there is no timer
//! interrupt, no heap, and no sound bank: the wavetables live in flash.
//!
//! ## Board mapping (NUCLEO-WBA65RI)
//!
//! | Signal | Pin  | Used for                                    |
//! |--------|------|---------------------------------------------|
//! | PA2    | TIM3_CH1 | PWM audio output (AF2)                  |
//! | PC13   | EXTI13   | USER button: cycle through sound cues   |
//! | PC4    | GPIO     | LD2 heartbeat                           |
//!
//! Wire PA2 through a series resistor and a capacitor to ground (an RC
//! low-pass, e.g. 1 kΩ + 4.7 nF) before the piezo/speaker. See `README.md`.
//!
//! [`embedded-audio`]: https://github.com/leftger/embedded-audio
//! [`embassy-stm32`]: https://github.com/embassy-rs/embassy

#![no_std]
#![no_main]

use defmt::{info, warn};
use defmt_rtt as _; // global defmt logger over RTT
use panic_probe as _; // panics go to defmt/RTT

use embassy_executor::Spawner;
use embassy_stm32::dma;
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::{Level, Output, OutputType, Pull, Speed};
use embassy_stm32::interrupt;
use embassy_stm32::peripherals;
use embassy_stm32::time::hz;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::{Config, Peri, bind_interrupts};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use static_cell::StaticCell;

use embedded_audio::prelude::*;

// ---------------------------------------------------------------------------
// Audio / DMA configuration
// ---------------------------------------------------------------------------

/// Audio sample rate. The PWM carrier runs at the same rate, so exactly one
/// duty word is written to `TIM3->CCR1` per PWM period.
const SAMPLE_RATE_HZ: u32 = 32_000;

/// GPDMA ring buffer, in 16-bit duty words. 512 words ≈ 16 ms of audio.
/// Must be even: the embassy GPDMA driver splits it into two ping-pong halves.
const DMA_BUF_WORDS: usize = 512;

/// Duty words rendered per loop iteration. It must divide `DMA_BUF_WORDS / 2`
/// (256) so that each half-transfer interrupt wakes the pump with room for a
/// whole chunk: 256 / 128 = 2 chunks per wake-up.
const CHUNK_WORDS: usize = 128;

/// Milliseconds of audio contained in one chunk (4 ms at 32 kHz).
const CHUNK_MS: u32 = (CHUNK_WORDS as u32 * 1000) / SAMPLE_RATE_HZ;

/// How often to log telemetry (2 s).
const TELEMETRY_CHUNKS: u32 = 2_000 / CHUNK_MS;

/// Mixer voices: three pad notes, their release tails, plus SFX headroom.
const VOICES: usize = 8;

/// Ambient pad: a three-note chord is re-triggered every `PAD_CHORD_CHUNKS`
/// (800 × 4 ms ≈ 3.2 s).
const PAD_CHORDS: [&[u16; 3]; 4] = [
    &[131, 165, 196], // C3  E3  G3
    &[110, 131, 165], // A2  C3  E3
    &[87, 110, 131],  // F2  A2  C3
    &[98, 123, 147],  // G2  B2  D3
];
const PAD_CHORD_CHUNKS: u32 = 800;
const PAD_GAIN_Q8: u8 = 70;
const PAD_PRIORITY: u8 = 60;
const PAD_FADE_MS: u16 = 500;
const PAD_ADSR: AdsrSpec = AdsrSpec {
    attack_ms: 250,
    decay_ms: 600,
    sustain_q8: 150,
    release_ms: PAD_FADE_MS,
};

/// One-shot SFX use a higher priority so they can steal pad voices, and a
/// short fade so cutting them never clicks.
const SFX_PRIORITY: u8 = 220;
const SFX_FADE_MS: u16 = 60;

// ---------------------------------------------------------------------------
// Interrupt bindings
// ---------------------------------------------------------------------------

bind_interrupts!(struct Irqs {
    GPDMA1_CHANNEL0 => dma::InterruptHandler<peripherals::GPDMA1_CH0>;
    EXTI13 => exti::InterruptHandler<interrupt::typelevel::EXTI13>;
});

// ---------------------------------------------------------------------------
// Sound cues
// ---------------------------------------------------------------------------

/// A cue is a short monophonic melody rendered on an allocated voice.
#[derive(Clone, Copy, defmt::Format)]
enum Cue {
    /// Startup jingle, played once from `main`.
    Boot,
    /// Two-note "coin" blip.
    Coin,
    /// Five-note rising chirp.
    Chirp,
    /// Descending saw "zap".
    Zap,
}

/// Which built-in flash wavetable a cue or pad is rendered with.
#[derive(Clone, Copy)]
enum Timbre {
    Sine,
    Triangle,
    Saw,
    Square,
}

impl Timbre {
    fn table(self) -> &'static [u8] {
        match self {
            Timbre::Sine => &SINE_TABLE,
            Timbre::Triangle => &TRIANGLE_TABLE,
            Timbre::Saw => &SAW_TABLE,
            Timbre::Square => &SQUARE_TABLE,
        }
    }
}

/// A monophonic melody: `notes` are triggered `spacing_ms` apart.
struct Sfx {
    notes: &'static [u16],
    spacing_ms: u16,
    adsr: AdsrSpec,
    gain_q8: u8,
    timbre: Timbre,
}

static BOOT: Sfx = Sfx {
    notes: &[523, 659, 784, 1047],
    spacing_ms: 110,
    adsr: AdsrSpec {
        attack_ms: 2,
        decay_ms: 240,
        sustain_q8: 0,
        release_ms: 50,
    },
    gain_q8: 150,
    timbre: Timbre::Sine,
};

static COIN: Sfx = Sfx {
    notes: &[988, 1319],
    spacing_ms: 70,
    adsr: AdsrSpec {
        attack_ms: 2,
        decay_ms: 160,
        sustain_q8: 0,
        release_ms: 40,
    },
    gain_q8: 180,
    timbre: Timbre::Square,
};

static CHIRP: Sfx = Sfx {
    notes: &[440, 587, 740, 880, 1109],
    spacing_ms: 50,
    adsr: AdsrSpec {
        attack_ms: 2,
        decay_ms: 130,
        sustain_q8: 0,
        release_ms: 40,
    },
    gain_q8: 150,
    timbre: Timbre::Triangle,
};

static ZAP: Sfx = Sfx {
    notes: &[220, 175, 131, 98],
    spacing_ms: 45,
    adsr: AdsrSpec {
        attack_ms: 2,
        decay_ms: 110,
        sustain_q8: 0,
        release_ms: 60,
    },
    gain_q8: 190,
    timbre: Timbre::Saw,
};

fn spec(cue: Cue) -> &'static Sfx {
    match cue {
        Cue::Boot => &BOOT,
        Cue::Coin => &COIN,
        Cue::Chirp => &CHIRP,
        Cue::Zap => &ZAP,
    }
}

/// Converts a duration in milliseconds to a (ceiled) number of chunks.
const fn chunks(ms: u16) -> u16 {
    let n = (ms as u32).div_ceil(CHUNK_MS);
    if n == 0 {
        1
    } else if n > u16::MAX as u32 {
        u16::MAX
    } else {
        n as u16
    }
}

/// Global cue queue: the button task produces, the audio pump consumes.
static SFX_CHANNEL: Channel<CriticalSectionRawMutex, Cue, 4> = Channel::new();

// ---------------------------------------------------------------------------
// Voice lifecycle
// ---------------------------------------------------------------------------

/// A voice that must be given back to the engine.
///
/// Wavetable sources loop forever, so a voice only becomes reusable after
/// `Voice::release()` (fade out) *and* `Voice::stop_immediate()` (stop the
/// source). Both are scheduled here, in chunks of audio.
#[derive(Clone, Copy)]
struct JanitorSlot {
    idx: usize,
    /// Chunks until `release()`; `ALREADY_RELEASED` if release was called eagerly.
    release_in: u16,
    /// Chunks until `stop_immediate()`, after which the slot is free again.
    stop_in: u16,
}

const FREE_SLOT: JanitorSlot = JanitorSlot {
    idx: usize::MAX,
    release_in: 0,
    stop_in: 0,
};
const ALREADY_RELEASED: u16 = u16::MAX;

struct VoiceJanitor {
    slots: [JanitorSlot; VOICES],
}

impl VoiceJanitor {
    fn new() -> Self {
        Self {
            slots: [FREE_SLOT; VOICES],
        }
    }

    /// Schedules `release()` after `hold_chunks`, then `stop_immediate()` after
    /// the `release_ms` fade that follows it.
    fn schedule_hold(&mut self, idx: usize, hold_chunks: u16, release_ms: u16) {
        let fade = chunks(release_ms);
        self.insert(JanitorSlot {
            idx,
            release_in: hold_chunks,
            stop_in: hold_chunks.saturating_add(fade).saturating_add(1),
        });
    }

    /// Schedules only the stop for a voice that was released eagerly.
    fn schedule_stop(&mut self, idx: usize, release_ms: u16) {
        self.insert(JanitorSlot {
            idx,
            release_in: ALREADY_RELEASED,
            stop_in: chunks(release_ms).saturating_add(1),
        });
    }

    fn insert(&mut self, slot: JanitorSlot) {
        if let Some(free) = self.slots.iter_mut().find(|s| s.idx == usize::MAX) {
            *free = slot;
        } else {
            // Every slot is in use: the voice would leak. This cannot happen
            // while `VOICES == slots.len()` and voices are only ever handed to
            // the janitor once, but log it rather than silently dropping it.
            warn!("voice janitor full, voice {} may leak", slot.idx);
        }
    }

    fn tick(&mut self, engine: &mut AudioEngine<'static, VOICES>) {
        for slot in &mut self.slots {
            if slot.idx == usize::MAX {
                continue;
            }

            if slot.release_in != ALREADY_RELEASED {
                if slot.release_in > 0 {
                    slot.release_in -= 1;
                } else {
                    if let Some(v) = engine.voice_mut(slot.idx) {
                        v.release();
                    }
                    slot.release_in = ALREADY_RELEASED;
                }
            }

            if slot.stop_in > 0 {
                slot.stop_in -= 1;
            } else {
                if let Some(v) = engine.voice_mut(slot.idx) {
                    v.stop_immediate();
                }
                slot.idx = usize::MAX;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Sequencer / player
// ---------------------------------------------------------------------------

struct Player {
    /// Melody currently playing, if any.
    cue: Option<&'static Sfx>,
    /// Index of the next note of `cue`.
    note: usize,
    /// Chunks left before the next note of `cue`.
    wait_chunks: u16,
    /// Index into [`PAD_CHORDS`] of the next chord.
    pad_chord: usize,
    /// Chunks left until the next ambient chord.
    pad_wait_chunks: u32,
    /// Voice indices currently held by the pad.
    pad_voices: [usize; 3],
    janitor: VoiceJanitor,
}

impl Player {
    fn new() -> Self {
        Self {
            cue: None,
            note: 0,
            wait_chunks: 0,
            pad_chord: 0,
            // Start the pad immediately.
            pad_wait_chunks: 0,
            pad_voices: [usize::MAX; 3],
            janitor: VoiceJanitor::new(),
        }
    }

    /// Queues a cue, replacing whatever melody is currently playing.
    fn start(&mut self, cue: Cue) {
        self.cue = Some(spec(cue));
        self.note = 0;
        self.wait_chunks = 0;
    }

    /// Called once per rendered chunk.
    fn tick(&mut self, engine: &mut AudioEngine<'static, VOICES>) {
        self.janitor.tick(engine);

        if let Some(sfx) = self.cue {
            if self.wait_chunks > 0 {
                self.wait_chunks -= 1;
            } else if self.note < sfx.notes.len() {
                let freq = sfx.notes[self.note];
                self.note += 1;
                self.wait_chunks = chunks(sfx.spacing_ms);
                if let Ok(idx) = engine.play_wavetable_with_priority(
                    sfx.timbre.table(),
                    freq as u32,
                    sfx.adsr,
                    SFX_PRIORITY,
                ) {
                    if let Some(v) = engine.voice_mut(idx) {
                        v.set_gain_q8(sfx.gain_q8);
                    }
                    self.janitor
                        .schedule_hold(idx, chunks(sfx.adsr.decay_ms), SFX_FADE_MS);
                }
            } else {
                self.cue = None;
            }
        }

        if self.pad_wait_chunks > 0 {
            self.pad_wait_chunks -= 1;
            return;
        }

        // Fade out the previous chord (it keeps sounding during the release).
        for i in 0..self.pad_voices.len() {
            let idx = core::mem::replace(&mut self.pad_voices[i], usize::MAX);
            if idx == usize::MAX {
                continue;
            }
            if let Some(v) = engine.voice_mut(idx) {
                v.release();
            }
            self.janitor.schedule_stop(idx, PAD_FADE_MS);
        }

        let chord = PAD_CHORDS[self.pad_chord % PAD_CHORDS.len()];
        self.pad_chord = self.pad_chord.wrapping_add(1);
        for (slot, &freq) in self.pad_voices.iter_mut().zip(chord.iter()) {
            if let Ok(idx) = engine.play_wavetable_with_priority(
                &SINE_TABLE,
                freq as u32,
                PAD_ADSR,
                PAD_PRIORITY,
            ) {
                if let Some(v) = engine.voice_mut(idx) {
                    v.set_gain_q8(PAD_GAIN_Q8);
                }
                *slot = idx;
            }
        }
        self.pad_wait_chunks = PAD_CHORD_CHUNKS;
    }
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

/// LD2 heartbeat, so it is obvious the executor is still running.
#[embassy_executor::task]
async fn heartbeat(mut led: Output<'static>) {
    loop {
        led.set_high();
        Timer::after_millis(60).await;
        led.set_low();
        Timer::after_millis(940).await;
    }
}

/// USER button (PC13) → non-blocking cue dispatch over [`SFX_CHANNEL`].
#[embassy_executor::task]
async fn user_button(
    pin: Peri<'static, peripherals::PC13>,
    exti: Peri<'static, peripherals::EXTI13>,
) {
    static CUES: [Cue; 3] = [Cue::Coin, Cue::Chirp, Cue::Zap];

    let mut button = ExtiInput::new(pin, exti, Pull::Up, Irqs);
    let mut next = 0;
    loop {
        button.wait_for_falling_edge().await;
        let cue = CUES[next];
        next = (next + 1) % CUES.len();
        info!("USER button -> {}", cue);
        // The queue is only full if cues arrive faster than they are consumed;
        // dropping one is the right behaviour for a UI sound.
        let _ = SFX_CHANNEL.try_send(cue);
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Config::default());
    info!("embedded-audio PWM+DMA showcase started");

    // Independent async tasks: no timer ISR, no critical sections in the audio path.
    // In embassy-executor 0.10 a task function returns `Result<SpawnToken, _>`
    // (it claims the task's static storage), so unwrap *that* before spawning.
    spawner.spawn(heartbeat(Output::new(p.PC4, Level::Low, Speed::Low)).unwrap());
    spawner.spawn(user_button(p.PC13, p.EXTI13).unwrap());

    // --- PWM carrier + GPDMA ring buffer ---------------------------------
    //
    // `SimplePwm` owns the timer and sets `ARR` for the requested frequency.
    // The channel output is enabled and then handed to the GPDMA driver, which
    // re-arms `CCR1` on every update event from a circular ring buffer.
    let ch1_pin = PwmPin::new(p.PA2, OutputType::PushPull);
    let mut pwm = SimplePwm::new(
        p.TIM3,
        Some(ch1_pin),
        None,
        None,
        None,
        hz(SAMPLE_RATE_HZ),
        Default::default(),
    );

    let mut ch1 = pwm.ch1();
    ch1.enable();
    // `max_duty_cycle()` is `ARR + 1` for this timer clock, i.e. the PWM period
    // in counts. Feed it to the engine so duties line up with the hardware.
    let period_counts = ch1.max_duty_cycle();
    let period = u16::try_from(period_counts).expect("PWM period does not fit in u16");
    info!(
        "TIM3_CH1: {} Hz carrier, duty period {} counts",
        SAMPLE_RATE_HZ, period_counts
    );

    let mut engine: AudioEngine<'static, VOICES> = AudioEngine::with_voice_count(AudioConfig::new(
        SAMPLE_RATE_HZ,
        period,
        DutyMode::SigmaDelta2ndOrder,
    ));
    engine.set_master_gain_q8(220);

    static DMA_BUF: StaticCell<[u16; DMA_BUF_WORDS]> = StaticCell::new();
    let dma_buf = DMA_BUF.init([0u16; DMA_BUF_WORDS]);
    let mut audio = ch1.into_ring_buffered_channel(p.GPDMA1_CH0, dma_buf, Irqs);
    audio.start();

    // --- Audio pump ------------------------------------------------------
    let mut player = Player::new();
    player.start(Cue::Boot);

    let mut chunk = [0u16; CHUNK_WORDS];
    let mut underruns: u32 = 0;
    let mut rendered_chunks: u32 = 0;

    loop {
        // Apply any cues queued by the button task since the last chunk.
        while let Ok(cue) = SFX_CHANNEL.try_receive() {
            player.start(cue);
        }

        // Advance envelopes/sequencers and trigger new notes.
        player.tick(&mut engine);

        // Render one chunk of PWM duty values.
        engine.fill_duty_buffer(&mut chunk);

        // Hand the chunk to the DMA ring buffer and sleep until the GPDMA has
        // drained enough space for the next one. `Err` means the DMA lapped the
        // writer (a real underrun); the ring buffer resets itself, so we just
        // count it and keep going.
        if audio.write_exact(&chunk).await.is_err() {
            underruns += 1;
        }

        rendered_chunks = rendered_chunks.wrapping_add(1);
        if rendered_chunks.is_multiple_of(TELEMETRY_CHUNKS) {
            let secs = (rendered_chunks as u64 * CHUNK_WORDS as u64) / SAMPLE_RATE_HZ as u64;
            info!(
                "up {} s | chunks {} | underruns {} | voices {}",
                secs,
                rendered_chunks,
                underruns,
                engine.active_voice_count()
            );
        }
    }
}
