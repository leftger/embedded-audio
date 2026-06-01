//! Host tool: build `EAFX` sound banks for flash programming.
//!
//! ```text
//! cargo run --features std --bin eaf-bake -- -o ui.bank --rate 16000 \
//!   --add 1:pcm8:click.raw \
//!   --add 2:tone:880:120 \
//!   --add 3:wavetable:440:lead256.raw
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;

use embedded_audio::encode::adpcm;
use embedded_audio::{BankBuilder, EffectKind, flags, AudioError, BANK_BUILD_CAP};

#[derive(Debug)]
struct AddSpec {
    id: u16,
    kind: String,
    args: Vec<String>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("eaf-bake: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let mut sample_rate = 16_000u32;
    let mut output = PathBuf::from("bank.bin");
    let mut adds: Vec<AddSpec> = Vec::new();

    // Legacy single-effect flags
    let mut legacy_kind = None;
    let mut legacy_id = 1u16;
    let mut legacy_input = None;
    let mut tone_hz = 440u32;
    let mut tone_ms = 200u16;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--rate" => {
                i += 1;
                sample_rate = args
                    .get(i)
                    .ok_or("--rate needs value")?
                    .parse()
                    .map_err(|_| "bad rate")?;
            }
            "-o" | "--output" => {
                i += 1;
                output = PathBuf::from(args.get(i).ok_or("-o needs path")?);
            }
            "--add" => {
                i += 1;
                let spec = args.get(i).ok_or("--add needs SPEC")?;
                adds.push(parse_add_spec(spec)?);
            }
            "--id" => {
                i += 1;
                legacy_id = args
                    .get(i)
                    .ok_or("--id needs value")?
                    .parse()
                    .map_err(|_| "bad id")?;
            }
            "--kind" => {
                i += 1;
                legacy_kind = Some(args.get(i).ok_or("--kind needs value")?.clone());
            }
            "--tone-hz" => {
                i += 1;
                tone_hz = args
                    .get(i)
                    .ok_or("--tone-hz needs value")?
                    .parse()
                    .map_err(|_| "bad hz")?;
            }
            "--tone-ms" => {
                i += 1;
                tone_ms = args
                    .get(i)
                    .ok_or("--tone-ms needs value")?
                    .parse()
                    .map_err(|_| "bad ms")?;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            path if !path.starts_with('-') => {
                legacy_input = Some(PathBuf::from(path));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    let mut builder = BankBuilder::new(sample_rate);

    if adds.is_empty() {
        let kind = legacy_kind.unwrap_or_else(|| "pcm8".to_string());
        let spec = match kind.as_str() {
            "pcm8" | "adpcm" | "tone" => {
                let path = legacy_input
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                let mut a = vec![];
                if kind != "tone" {
                    a.push(path);
                }
                AddSpec {
                    id: legacy_id,
                    kind: kind.clone(),
                    args: if kind == "tone" {
                        vec![tone_hz.to_string(), tone_ms.to_string()]
                    } else {
                        a
                    },
                }
            }
            other => return Err(format!("unknown kind: {other}")),
        };
        adds.push(spec);
    }

    for spec in &adds {
        apply_add(&mut builder, spec)?;
    }

    let mut out = heapless::Vec::<u8, BANK_BUILD_CAP>::new();
    builder
        .finish(&mut out)
        .map_err(|e: AudioError| e.as_str().to_string())?;
    fs::write(&output, &out).map_err(|e| e.to_string())?;
    println!(
        "Wrote {} bytes, {} Hz, {} effects → {}",
        out.len(),
        sample_rate,
        adds.len(),
        output.display()
    );
    Ok(())
}

fn parse_add_spec(spec: &str) -> Result<AddSpec, String> {
    // id:kind:arg0:arg1...
    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() < 2 {
        return Err(format!(
            "bad --add SPEC '{spec}' (want id:kind:args..., e.g. 1:pcm8:file.raw)"
        ));
    }
    let id: u16 = parts[0].parse().map_err(|_| "bad effect id")?;
    let kind = parts[1].to_string();
    let args: Vec<String> = parts[2..].iter().map(|s| (*s).to_string()).collect();
    Ok(AddSpec { id, kind, args })
}

fn apply_add(builder: &mut BankBuilder, spec: &AddSpec) -> Result<(), String> {
    match spec.kind.as_str() {
        "pcm8" => {
            let path = spec.args.first().ok_or("pcm8 needs file path")?;
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            builder
                .add_effect(
                    spec.id,
                    EffectKind::Pcm8,
                    flags::ONE_SHOT,
                    255,
                    0,
                    0,
                    &bytes,
                )
                .map_err(|e| e.as_str().to_string())?;
        }
        "adpcm" => {
            let path = spec.args.first().ok_or("adpcm needs file path")?;
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            let payload = adpcm::encode_u8(&bytes);
            builder
                .add_effect(
                    spec.id,
                    EffectKind::Adpcm,
                    flags::ONE_SHOT,
                    255,
                    0,
                    0,
                    &payload,
                )
                .map_err(|e| e.as_str().to_string())?;
        }
        "tone" => {
            let hz: u32 = spec
                .args
                .first()
                .ok_or("tone needs hz")?
                .parse()
                .map_err(|_| "bad hz")?;
            let ms: u16 = spec
                .args
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or("200")
                .parse()
                .map_err(|_| "bad ms")?;
            builder
                .add_effect(
                    spec.id,
                    EffectKind::Tone,
                    flags::ONE_SHOT,
                    255,
                    hz.min(u16::MAX as u32) as u16,
                    ms,
                    &[],
                )
                .map_err(|e| e.as_str().to_string())?;
        }
        "wavetable" => {
            let hz: u32 = spec
                .args
                .first()
                .ok_or("wavetable needs hz:file")?
                .parse()
                .map_err(|_| "bad hz")?;
            let path = spec.args.get(1).ok_or("wavetable needs 256-byte table file")?;
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            if bytes.len() < 256 {
                return Err(format!(
                    "wavetable file {} must be at least 256 bytes (got {})",
                    path,
                    bytes.len()
                ));
            }
            builder
                .add_effect(
                    spec.id,
                    EffectKind::Wavetable,
                    flags::ONE_SHOT,
                    255,
                    hz.min(u16::MAX as u32) as u16,
                    0,
                    &bytes,
                )
                .map_err(|e| e.as_str().to_string())?;
        }
        "fm" => {
            let hz: u32 = spec
                .args
                .first()
                .ok_or("fm needs carrier_hz")?
                .parse()
                .map_err(|_| "bad hz")?;
            let ratio: u16 = spec
                .args
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or("100")
                .parse()
                .map_err(|_| "bad mod ratio")?;
            builder
                .add_effect(
                    spec.id,
                    EffectKind::Fm,
                    flags::ONE_SHOT,
                    255,
                    hz.min(u16::MAX as u32) as u16,
                    ratio,
                    &[],
                )
                .map_err(|e| e.as_str().to_string())?;
        }
        other => return Err(format!("unknown kind in --add: {other}")),
    }
    println!("  + effect {} ({})", spec.id, spec.kind);
    Ok(())
}

fn print_help() {
    println!(
        "eaf-bake — build EAFX v2 sound banks\n\n\
         Multi-effect:\n  \
         eaf-bake -o ui.bank --rate 16000 \\\n    \
         --add 1:pcm8:click.raw \\\n    \
         --add 2:adpcm:whoosh.raw \\\n    \
         --add 3:tone:880:100 \\\n    \
         --add 4:wavetable:440:table256.raw \\\n    \
         --add 5:fm:520:200\n\n\
         Legacy single-effect:\n  \
         eaf-bake --kind pcm8 --id 1 click.raw -o bank.bin\n\n\
         Kinds: pcm8, adpcm, tone, wavetable (256-byte file), fm\n\
         Input for pcm8/adpcm: unsigned 8-bit mono raw, centered at 128.\n"
    );
}
