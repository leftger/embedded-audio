# Embassy & Hardware Peripherals Integration Guide

This directory contains examples for driving hardware peripherals (DAC, PWM, I2S/SAI) using `embedded-audio` and `embassy-stm32` with DMA on microcontrollers such as the **STM32U585CIU6**.

## Running host examples

```bash
cargo run --example dac_dma_stream
```

---

## STM32U585CIU6 Embassy Async DMA Driver Example

The snippet below demonstrates continuous async DMA streaming to **DAC1** triggered by **TIM6** on an STM32U585 microcontroller using Embassy:

```rust
//! Embassy STM32U585CIU6 Audio Output Example with `embedded-audio`
//!
//! Demonstrates streaming synthesized wavetables and audio engine ticks
//! via DMA to DAC, PWM, or SAI peripherals on STM32U585.

#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_stm32::dac::{DacChannel, Value};
use embassy_stm32::peripherals::GPDMA1_CH0;
use embassy_stm32::rcc::{LsConfig, enable_and_reset, mux};
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::low_level::{MasterMode, RoundTo, Timer};
use embassy_stm32::triggers::TIM6_TRGO;
use embassy_stm32::{Config, bind_interrupts, dma};

use embedded_audio::prelude::*;

bind_interrupts!(struct Irqs {
    GPDMA1_CHANNEL0 => dma::InterruptHandler<GPDMA1_CH0>;
});

/// Sample rate for audio synthesis (16 kHz)
const SAMPLE_RATE_HZ: u32 = 16_000;
/// Size of each DMA half-buffer (samples per channel chunk)
const DMA_HALF_SIZE: usize = 256;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Initializing STM32U585CIU6 Audio Engine with Embassy...");

    let mut config = Config::default();
    config.rcc.ls = LsConfig::default_lsi();
    config.rcc.mux.dac1sel = mux::Dacsel::Lsi;
    let p = embassy_stm32::init(config);

    // 1. Setup embedded-audio Engine & Wavetables
    let mut engine = AudioEngine::from_sample_rate(SAMPLE_RATE_HZ, 4095, DutyMode::Linear);

    // Custom wavetable using fixed-point generator:
    let custom_wavetable = generate_wavetable_fixed(|idx| {
        if idx < 128 { (idx as i16 - 64) as i8 } else { (191 - idx as i16) as i8 }
    });

    let adsr = AdsrSpec {
        attack_ms: 20,
        decay_ms: 100,
        sustain_q8: 180,
        release_ms: 300,
    };
    engine.play_wavetable(&custom_wavetable, 440, adsr).unwrap();
    engine.play_wavetable(&TRIANGLE_TABLE, 554, adsr).unwrap();

    // 2. Setup Peripheral & DMA (DAC1 with TIM6 trigger on PA4)
    let mut dac = DacChannel::new_triggered(p.DAC1, p.GPDMA1_CH0, TIM6_TRGO, Irqs, p.PA4);
    enable_and_reset::<embassy_stm32::peripherals::TIM6>();

    let timer = Timer::new(p.TIM6);
    timer.set_frequency(Hertz(SAMPLE_RATE_HZ), RoundTo::Faster);
    timer.set_master_mode(MasterMode::Update);
    timer.start();

    // 3. Double-Buffered Async DMA Audio Pump
    let mut dma_pump = DmaDoubleBuffer::<u16, DMA_HALF_SIZE>::new();

    loop {
        // Get next buffer half and fill with 12-bit DAC values (0..=4095)
        let buf = dma_pump.swap_and_get_next();
        engine.fill_dac_u12_buffer(buf);

        let dac_values: &[Value] = unsafe {
            core::slice::from_raw_parts(buf.as_ptr() as *const Value, buf.len())
        };

        // Await DMA transfer completion for active buffer chunk
        dac.write(dac_values).await;
    }
}
```
