# PWM audio over GPDMA on STM32WBA65RI (embassy showcase)

A complete, buildable firmware example that drives **PWM audio on an
STM32WBA65RI** using **GPDMA** and the [`embassy`] async framework, with the
[`embedded-audio`] crate doing the synthesis.

It is the "reference" hardware example for this workspace: everything else in
`crates/` is host-side or library code, while this crate is a real `no_std`
firmware image you can flash and hear.

## What it demonstrates

- A fixed **32 kHz PWM carrier** on `TIM3_CH1` (PA2) modulated by a continuous
  stream of duty-cycle values.
- **Zero-CPU audio transport**: `embassy-stm32`'s
  `SimplePwmChannel::into_ring_buffered_channel()` sets up a GPDMA ring buffer
  writing straight into `TIM3->CCR1`, and `write_exact().await` parks the task
  until the DMA has drained enough space for the next chunk. No timer
  interrupt, no DMA-completion ISR, no heap.
- A **polyphonic mixer** from `embedded-audio`: wavetable oscillators, ADSR
  envelopes, an 8-voice pool with priority-based voice stealing, and
  second-order sigma-delta noise shaping on the PWM output.
- **Async task composition**: a button task dispatches sound cues to the audio
  task over an `embassy-sync` channel, and a heartbeat task drives the LED —
  all independent of the audio path.
- **Correct voice lifecycle** for looping wavetables: voices are `release()`d
  and then `stop_immediate()`d so the pool is never leaked (see
  [Voice lifecycle](#voice-lifecycle)).

## Hardware

### NUCLEO-WBA65RI

| Signal          | Pin  | Peripheral      | Notes                                        |
|-----------------|------|-----------------|----------------------------------------------|
| PWM audio out   | PA2  | `TIM3_CH1` (AF2)| Carrier = 32 kHz, sample rate = 32 kHz       |
| USER button B1  | PC13 | `EXTI13`        | Active low, internal pull-up, falling edge   |
| LED LD2         | PC4  | GPIO            | Heartbeat, ~1 Hz                             |

PA2 is the pin used by embassy's own `stm32wba6` PWM example, so it is known to
be broken out on the board headers.

### Output filter

PWM audio needs a low-pass filter to remove the 32 kHz carrier. A cheap
first-order RC is usually enough for a piezo:

```text
PA2 ──[ R 1 kΩ ]──┬────────────► piezo +
                  │
              [ C 4.7 nF ]          piezo − ── GND
                  │
                 GND
```

That puts the corner at roughly 34 kHz, which keeps the audio band (up to
~16 kHz) and attenuates the carrier. Piezo elements are partly self-filtering,
so you can often drive one through just a series resistor (100 Ω – 1 kΩ).

For a small **magnetic** speaker you need more drive than a GPIO can provide:
use a logic-level MOSFET or a small Class-D module, keep the 32 kHz carrier
filter (a series capacitor blocks the DC), and never connect a speaker
directly across the pin.

## Build & flash

```bash
# once
rustup target add thumbv8m.main-none-eabihf

cd examples/stm32wba65ri-pwm-audio
cargo run --release      # uses `probe-rs run --chip STM32WBA65RI`
```

`cargo run` in this directory picks up `.cargo/config.toml`, which selects the
`thumbv8m.main-none-eabihf` target and the probe-rs runner. To build from the
repository root, pass the target explicitly:

```bash
# from the repository root
cargo build -p stm32wba65ri-pwm-audio --target thumbv8m.main-none-eabihf --release
probe-rs run --chip STM32WBA65RI \
    target/thumbv8m.main-none-eabihf/release/stm32wba65ri-pwm-audio
```

RTT logs are enabled by default (`DEFMT_LOG=info`). Adjust it in
`.cargo/config.toml`, e.g. `DEFMT_LOG=debug`, or set the env var when building.

Because the crate is a workspace member but cannot be built for the host, the
root workspace uses `default-members` to keep `cargo build` / `cargo test` on
the host toolchain working. CI checks it with a dedicated `check-firmware` job.

### Dependencies

The `embassy-*` crates are pulled from **git `main`** (`embassy-rs/embassy`)
rather than crates.io: the newest published `embassy-stm32` is 0.6.0 from March
2026 and trails `main` by months. There is deliberately no `rev` pin, so the
first build needs network access and an upstream change can break the build; add
`rev = "<sha>"` to all four embassy lines in `Cargo.toml` if you want
reproducible firmware.

Note that `embassy-stm32@main` has a non-optional dependency on
`xarxa-driver` (from `embassy-rs/xarxa`, `0BSD`), which is why `deny.toml`
allows that license and git source.

## How it works

```text
  Player (cue + chord sequencer)
        │  play_wavetable_with_priority() / voice_mut()
        ▼
  AudioEngine<'static, 8>            (wavetables → ADSR → mix → ΣΔ → duty)
        │  fill_duty_buffer(&mut chunk: [u16; 128])
        ▼
  RingBufferedPwmChannel::write_exact().await
        │  GPDMA1_CH0: memory → TIM3->CCR1, circular, on every TIM3 update event
        ▼
  TIM3_CH1 (PA2) ── 32 kHz PWM ──► RC filter ──► piezo
```

### Timing

The PWM carrier and the audio sample rate are the **same 32 kHz**, so exactly
one duty word is transferred per PWM period and the DMA rate *is* the sample
rate. That keeps the whole path to a single, easily reasoned multiply: one
`AudioEngine` tick == one PWM period.

| Quantity                  | Value | Derivation                          |
|---------------------------|-------|-------------------------------------|
| Sample rate / carrier     | 32 kHz| `SAMPLE_RATE_HZ`                    |
| Audio bandwidth           | ~16 kHz | Nyquist                           |
| Ring buffer               | 512 words | ≈ 16 ms of audio (`DMA_BUF_WORDS`) |
| Chunk rendered per wake-up| 128 words | 4 ms (`CHUNK_WORDS`)            |
| Buffered latency          | ≤ 16 ms | one full ring buffer              |

The GPDMA driver splits the ring buffer into two ping-pong halves and wakes the
task on each half/full transfer, so `CHUNK_WORDS` must divide
`DMA_BUF_WORDS / 2` (256 / 128 = 2 chunks per wake-up). The DMA keeps the
buffer full, which is why the writer `await`s instead of polling: an underrun
would only happen if the renderer stalled for the whole buffer duration.

### Interrupts

Only two interrupt lines are bound, both by the HAL, not by the audio code:

```rust
bind_interrupts!(struct Irqs {
    GPDMA1_CHANNEL0 => dma::InterruptHandler<peripherals::GPDMA1_CH0>;
    EXTI13 => exti::InterruptHandler<interrupt::typelevel::EXTI13>;
});
```

The GPDMA interrupt only wakes the task; the audio is already sitting in the
ring buffer being clocked out by the timer.

### Voice lifecycle

`embedded-audio`'s wavetable sources loop forever, so a voice only becomes
reusable after **two** steps: `Voice::release()` (fade out via the ADSR) and
then `Voice::stop_immediate()` (stop the source). This example tracks every
voice it hands out in a small `VoiceJanitor` that schedules both, which is why
the voice pool stays healthy indefinitely. If you drop this bookkeeping,
`play_wavetable()` voices accumulate until the allocator runs out.

### Controls

| Event            | Result                                                        |
|------------------|---------------------------------------------------------------|
| Reset / boot     | Startup jingle (C5–E5–G5–C6), then the ambient pad starts     |
| USER button (1×) | "Coin" blip (two square notes)                                |
| USER button (2×) | "Chirp" (five-note triangle arpeggio)                         |
| USER button (3×) | "Zap" (descending saw)                                        |
| LED              | ~1 Hz heartbeat, shows the executor is alive                  |

The pad plays a four-chord progression (C–Am–F–G in the low octave) and
re-triggers every 3.2 s, fading the previous chord out.

## Tuning knobs

All in `src/main.rs`:

| Constant                                 | Purpose                                              |
|------------------------------------------|------------------------------------------------------|
| `SAMPLE_RATE_HZ`                         | PWM carrier and audio rate (change with `DMA_*` too) |
| `DMA_BUF_WORDS` / `CHUNK_WORDS`          | Latency vs. underrun headroom                        |
| `VOICES`                                 | Mixer polyphony                                      |
| `DutyMode::SigmaDelta2ndOrder`           | Try `Linear` (no shaping) or `SigmaDelta`            |
| `PAD_CHORDS` / `PAD_CHORD_CHUNKS`        | Ambient progression                                  |
| `BOOT` / `COIN` / `CHIRP` / `ZAP`        | Cue melodies, timbre, envelopes, gain                |

`AudioConfig::new(SAMPLE_RATE_HZ, period, mode)` takes the PWM period in timer
counts; the example reads it back from `ch1.max_duty_cycle()` so duties always
match the timer clock the HAL configured.

## Resource usage

Release build for `thumbv8m.main-none-eabihf` (default LTO-free profile):

```text
   text    data     bss     dec
  35040      80    3800   38920
```

i.e. about **34 KiB of flash** and **3.7 KiB of RAM**, most of which is
`defmt`/panic machinery and the asynchronous HAL. The audio engine itself is
well under 1 KiB.

## Porting to a different STM32

The audio side is portable; the hardware side is three substitutions:

1. Pick any general-purpose timer channel with an `UP` DMA request
   (`TIM1/2/3/4` all have one on WBA) and a 16- or 32-bit compare register.
2. Replace `SimplePwm::new(p.TIM3, …)` / `p.GPDMA1_CH0` / the `Irqs` bindings
   with the equivalent peripherals for your chip.
3. Keep `max_duty_cycle()` → `AudioConfig::pwm_period`, and keep using a `u16`
   duty buffer (`Word + Into<T::Word>` also allows `u32`/`u8` where supported).

Everything else — the engine, the player, the janitor — is chip-independent.

## Troubleshooting

- **Silence but RTT logs look fine** — check the DUTY wire: the DMA is writing
  `CCR1`, but you need PA2 (not another channel) and an RC filter for anything
  but a piezo.
- **"voice janitor full" warning** — you rendered cues faster than voices free
  up; reduce cue spacing or raise `VOICES`.
- **Underrun counter climbing** — debug logging or a long critical section is
  blocking the pump; shorten it or increase `DMA_BUF_WORDS`.
- **`probe-rs` cannot find the chip** — confirm `probe-rs chip list | grep -i
  WBA65`; adjust the `runner` in `.cargo/config.toml` if your probe-rs version
  spells the target differently.

[`embassy`]: https://embassy.dev
[`embedded-audio`]: https://github.com/leftger/embedded-audio
