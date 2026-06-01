//! Minimal mono WAV writer (8-bit unsigned PCM).

extern crate alloc;

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

/// Write mono 8-bit unsigned PCM WAV (`samples`: 0..=255, silence ≈ 128).
pub fn write_mono_u8(path: &str, sample_rate_hz: u32, samples: &[u8]) -> Result<(), String> {
    let data = build_wav_u8(sample_rate_hz, samples);
    std::fs::write(path, &data).map_err(|e| e.to_string())
}

pub fn build_wav_u8(sample_rate_hz: u32, samples: &[u8]) -> Vec<u8> {
    let byte_rate = sample_rate_hz * 1;
    let block_align = 1u16;
    let data_len = samples.len() as u32;
    let riff_len = 36 + data_len;

    let mut out = Vec::with_capacity(44 + samples.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate_hz.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(samples);
    out
}

/// Convert signed 8-bit engine samples to unsigned WAV bytes.
pub fn pcm_i8_to_u8(samples: &[i8]) -> Vec<u8> {
    samples.iter().map(|&s| (s as i16 + 128) as u8).collect()
}
