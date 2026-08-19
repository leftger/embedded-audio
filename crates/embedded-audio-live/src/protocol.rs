//! Binary / JSON protocol messages for live auditioning & hardware-in-the-loop streaming.

use serde::{Deserialize, Serialize};

/// Packet types sent between the host DAW and the embedded device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LiveAudioPacket {
    /// Host ping to verify device connection.
    Ping,
    /// Device pong acknowledging ping with device info.
    Pong {
        board_name: String,
        firmware_version: String,
        sample_rate_hz: u32,
        max_voices: u8,
    },
    /// Trigger a note on the target hardware.
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
        frequency_hz: f32,
    },
    /// Release a note on the target hardware.
    NoteOff { channel: u8, note: u8 },
    /// Update synth parameters in real-time.
    SetParam {
        channel: u8,
        param_id: u8,
        value_q8: u8,
    },
    /// Stream a chunk of 8-bit PCM audio samples.
    StreamPcmChunk {
        sample_rate_hz: u32,
        samples: Vec<i8>,
    },
    /// Stream a chunk of duty-cycle PWM values.
    StreamPwmDutyChunk { duty_values: Vec<u8> },
    /// Stop all audio immediately.
    AllNotesOff,
    /// Hardware telemetry reported by MCU to host DAW.
    Telemetry {
        cpu_usage_pct: u8,
        buffer_underruns: u32,
        free_heap_bytes: u32,
    },
}

impl LiveAudioPacket {
    /// Serialize packet to JSON bytes with newline delimiter.
    pub fn encode_framed(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Attempt to parse packet from framed buffer.
    pub fn decode_framed(buf: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(buf)
    }
}
