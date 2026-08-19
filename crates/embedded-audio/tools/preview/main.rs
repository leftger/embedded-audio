//! Render a bank effect to WAV using the same engine PCM path as firmware.
//!
//! ```text
//! cargo run --features std --bin eaf-preview -- --bank ui.bank --id 1 -o preview.wav
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;

use embedded_audio::encode::wav::{pcm_i8_to_u8, write_mono_u8};
use embedded_audio::{AdsrSpec, AudioConfig, AudioEngine, SoundBank};

fn main() {
    if let Err(e) = run() {
        eprintln!("eaf-preview: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let mut bank_path = PathBuf::from("bank.bin");
    let mut effect_id = 1u16;
    let mut output = PathBuf::from("preview.wav");
    let mut max_ms = 5000u32;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--bank" => {
                i += 1;
                bank_path = PathBuf::from(args.get(i).ok_or("--bank needs path")?);
            }
            "--id" => {
                i += 1;
                effect_id = args
                    .get(i)
                    .ok_or("--id needs value")?
                    .parse()
                    .map_err(|_| "bad id")?;
            }
            "-o" | "--output" => {
                i += 1;
                output = PathBuf::from(args.get(i).ok_or("-o needs path")?);
            }
            "--max-ms" => {
                i += 1;
                max_ms = args
                    .get(i)
                    .ok_or("--max-ms needs value")?
                    .parse()
                    .map_err(|_| "bad ms")?;
            }
            "--help" | "-h" => {
                println!(
                    "eaf-preview — render effect to WAV\n\n\
                     Usage: eaf-preview --bank ui.bank --id 1 -o out.wav [--max-ms 5000]\n"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    let blob = fs::read(&bank_path).map_err(|e| e.to_string())?;
    let bank = SoundBank::parse(&blob).map_err(|e| e.as_str().to_string())?;
    let rate = bank.sample_rate_hz;
    let max_samples = ((max_ms * rate) / 1000) as usize;

    let mut engine = AudioEngine::new(AudioConfig::default_duty().master_gain_q8(255));
    engine.set_bank(bank);
    engine
        .play(effect_id, AdsrSpec::click())
        .map_err(|e| e.as_str().to_string())?;

    let mut pcm = Vec::with_capacity(max_samples.min(256 * 1024));
    while engine.is_playing() && pcm.len() < max_samples {
        pcm.push(engine.tick_pcm());
    }

    let u8_samples = pcm_i8_to_u8(&pcm);
    write_mono_u8(
        output.to_str().ok_or("non-utf8 output path")?,
        rate,
        &u8_samples,
    )?;

    println!(
        "Rendered {} samples ({:.2} s) → {}",
        pcm.len(),
        pcm.len() as f64 / rate as f64,
        output.display()
    );
    Ok(())
}
