# embedded-audio

[![crates.io](https://img.shields.io/crates/v/embedded-audio.svg)](https://crates.io/crates/embedded-audio)
[![docs.rs](https://img.shields.io/docsrs/embedded-audio)](https://docs.rs/embedded-audio)
[![CI](https://github.com/leftger/embedded-audio/actions/workflows/ci.yml/badge.svg)](https://github.com/leftger/embedded-audio/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

`no_std` duty-modulated PWM audio for Cortex-M class MCUs: effect banks in external flash, tiered DSP, two-voice mixing, and crossfades.

## Architecture

```text
Flash (EAFX bank) → decode (Tier A/B/C) → 2 voices + ADSR → mix → ΣΔ → PWM duty
```

Call `AudioEngine::tick()` once per sample at `AudioConfig::sample_rate_hz` (default **16 kHz**). Drive a **fixed** PWM carrier (default **32 kHz** timer) by writing the returned duty compare value.

## Quick start (firmware)

```rust
use embedded_audio::prelude::*;

static BANK: &[u8] = include_bytes!("../assets/ui.bank");

let bank = SoundBank::parse(BANK)?;
let mut engine = AudioEngine::new(AudioConfig::default_duty());
engine.set_bank(bank);
engine.play(1, AdsrSpec::click())?;

// Timer ISR @ 16 kHz:
let duty = engine.tick();
pwm.set_duty(duty);
```

## Effect bank (`EAFX` v2)

| Offset | Field |
|--------|--------|
| 0..4 | Magic `EAFX` |
| 4 | Version `2` |
| 5..6 | Effect count (u16 LE) |
| 7..8 | Sample rate Hz (u16 LE) |
| 9 | Reserved |
| 10+ | Directory entries (16 bytes × N) |
| … | Payload bytes |

### Effect kinds

| ID | Tier | Payload |
|----|------|---------|
| `Tone` | A | empty — `param0` = Hz, `param1` = duration ms |
| `Wavetable` | A | 256-byte table |
| `Fm` | A | empty — `param0` = carrier Hz, `param1` = mod ratio ×100 |
| `Pcm8` | B | raw 8-bit mono |
| `Adpcm` | B | 4-byte IMA header + nibbles |
| `SigmaDeltaBits` | C | MSB-first packed bits |

## Host baking

Multi-effect bank:

```bash
cargo run --features std --bin eaf-bake -- -o ui.bank --rate 16000 \
  --add 1:pcm8:click.raw \
  --add 2:adpcm:whoosh.raw \
  --add 3:tone:880:120 \
  --add 4:wavetable:440:table256.raw
```

Legacy single effect: `eaf-bake --kind pcm8 --id 1 sound.raw -o bank.bin`

Input for `pcm8` / `adpcm` is **unsigned 8-bit mono** raw (center 128), band-limited in your DAW before export.  
`wavetable` requires a **256-byte** table file; `param0` in the bank is playback Hz.

## Preview (host)

```bash
cargo run --features std --bin eaf-preview -- --bank ui.bank --id 1 -o preview.wav
```

Uses `tick_pcm()` — the same mix/envelope path as firmware, before PWM mapping.

## Peripheral DMA & Format Buffering

`embedded-audio` supports arbitrary hardware peripherals (PWM timers, 8/12/16-bit DACs, I2S / SAI audio codecs) via peripheral-agnostic buffer fillers:

```rust
// Fill DMA buffers for different peripherals:
engine.fill_duty_buffer(&mut duty_buf);      // PWM timers (0..=period)
engine.fill_dac_u8_buffer(&mut dac_u8_buf);    // 8-bit DACs (0..=255)
engine.fill_dac_u12_buffer(&mut dac_u12_buf);  // 12-bit DACs (0..=4095, e.g. STM32 DAC1)
engine.fill_dac_u16_buffer(&mut dac_u16_buf);  // 16-bit DACs (0..=65535)
engine.fill_pcm_i16_buffer(&mut pcm_i16_buf);  // Signed 16-bit PCM (-32768..=32767)
engine.fill_stereo_i16_buffer(&mut stereo_buf);// Interleaved stereo i16 for I2S/SAI
```

## Embassy Async DMA Integration

Use `DmaDoubleBuffer` for zero-allocation ping-pong DMA streaming with Embassy async drivers:

```rust
use embedded_audio::prelude::*;

// 256 samples per half-buffer
let mut dma_pump = DmaDoubleBuffer::<u16, 256>::new();

loop {
    let buf = dma_pump.swap_and_get_next();
    engine.fill_dac_u12_buffer(buf);

    // Push via Embassy DMA write:
    dac.write(buf).await;
}
```

See [examples/embassy_stm32u585.rs](examples/embassy_stm32u585.rs) for a complete Embassy STM32U585CIU6 hardware example.

## Wavetables & Synthesizers

Play standard synthesized waveforms or custom 256-sample wavetables:

```rust
// Standard built-in wavetables: SINE_TABLE, TRIANGLE_TABLE, SAW_TABLE, SQUARE_TABLE, PULSE_25_TABLE
engine.play_wavetable(&SAW_TABLE, 440, AdsrSpec::click())?;

// Custom fixed-point wavetable generator:
let custom_table = generate_wavetable_fixed(|phase_idx| {
    // phase_idx: 0..255 -> return signed 8-bit sample -128..=127
    (phase_idx as i16 - 128) as i8
});
engine.play_wavetable(&custom_table, 880, AdsrSpec::click())?;
```

## Defaults


| Constant | Value |
|----------|-------|
| `DEFAULT_SAMPLE_RATE_HZ` | 16_000 |
| `DEFAULT_PWM_CARRIER_HZ` | 32_000 |
| `DEFAULT_PWM_PERIOD` | 1000 (set from your timer clock) |
| Output shaping | `DutyMode::SigmaDelta` |

## Real-time DSP & Analysis (`dsp` feature)

Enable the optional `dsp` feature to integrate zero-allocation digital signal processing algorithms powered by `embedded-dsp` (using `libm` for `#![no_std]` targets):

```toml
[dependencies]
embedded-audio = { version = "0.2.0", features = ["dsp"] }
```

```rust
use embedded_audio::prelude::*;
use embedded_audio::synth::Waveform;

let mut engine = AudioEngine::from_sample_rate(16000, 255, DutyMode::Linear);
engine.play_tone(440, 100, Waveform::Sine);

// 1. Equalize or filter engine audio with a Biquad filter (Lowpass, Highpass, Bandpass, Notch)
let mut filter = BiquadAudioFilter::lowpass(1000.0, 16000.0, 0.707);

let mut frame = [0.0f32; 256];
engine.fill_pcm_f32_buffer(&mut frame);
filter.process_buffer(&mut frame);

// 2. Measure audio signal statistics (RMS, Peak, Power, Mean, Variance)
let stats = AudioMeter::measure(&frame);
// stats.rms, stats.peak, stats.power, etc.

// 3. FFT Spectrum Analysis & Pitch / Dominant Frequency Detection
let (peak_freq_hz, peak_mag) = AudioSpectrumAnalyzer::find_peak_frequency(
    &frame,
    16000.0,
    WindowType::Hanning,
);
```

## Features

| Feature | Purpose |
|---------|---------|
| `std` | Host ADPCM encoder + `eaf-bake` binary |
| `fm` | Optional FM-buzzer backend (`tick_fm`, Markham profile) for bring-up only |
| `dsp` | Optional real-time DSP (`BiquadAudioFilter`, `AudioSpectrumAnalyzer`, `AudioMeter`, `AudioLmsFilter`) via `embedded-dsp` |

## RAM budget (typical)

- 2 voices + ADSR + ΣΔ state: **< 512 B**
- No heap; bank parsed from flash by reference

## License

The contents of this repository are dual-licensed under the _MIT OR Apache 2.0_
License. That means you can choose either the MIT license or the Apache 2.0
license when you re-use this code. See [`LICENSE-MIT`](./LICENSE-MIT) or
[`LICENSE-APACHE`](./LICENSE-APACHE) for more information on each specific
license.
