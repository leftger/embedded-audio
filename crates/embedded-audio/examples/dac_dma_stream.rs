use embedded_audio::prelude::*;

fn main() {
    println!("Embedded Audio DMA Streaming Example");

    // Initialize 2-voice audio engine at 16 kHz sample rate and 12-bit DAC resolution (0..4095)
    let mut engine = AudioEngine::from_sample_rate(16_000, 4095, DutyMode::Linear);

    // Play sawtooth wavetable tone at 440 Hz
    engine
        .play_wavetable(&SAW_TABLE, 440, AdsrSpec::click())
        .expect("Failed to start wavetable voice 0");

    // Play triangle wave harmony at 554 Hz
    engine
        .play_wavetable(&TRIANGLE_TABLE, 554, AdsrSpec::click())
        .expect("Failed to start wavetable voice 1");

    // Set up double-buffered DMA stream (256 samples per half-buffer)
    let mut dma_pump = DmaDoubleBuffer::<u16, 256>::new();

    println!("Simulating 5 DMA transfer buffers (12-bit DAC values)...");

    for i in 0..5 {
        let buf = dma_pump.swap_and_get_next();
        let samples_filled = engine.fill_dac_u12_buffer(buf);
        println!(
            "Buffer #{}: filled {} samples. First 4 DAC values: {:?}",
            i,
            samples_filled,
            &buf[0..4]
        );
    }
}
