//! Live audio streaming & hardware-in-the-loop link for embedded-audio.

pub mod protocol;

pub use protocol::LiveAudioPacket;

/// A mock transport for testing live streaming and loopback emulation.
pub struct MockDeviceBridge {
    pub connected: bool,
    pub board_name: String,
    pub sample_rate_hz: u32,
    pub max_voices: u8,
    pub sent_packets: Vec<LiveAudioPacket>,
}

impl Default for MockDeviceBridge {
    fn default() -> Self {
        Self {
            connected: true,
            board_name: "RP2040 PWM Audio Sink (Simulated)".to_string(),
            sample_rate_hz: 16000,
            max_voices: 4,
            sent_packets: Vec::new(),
        }
    }
}

impl MockDeviceBridge {
    pub fn new(board_name: &str) -> Self {
        Self {
            connected: true,
            board_name: board_name.to_string(),
            sample_rate_hz: 16000,
            max_voices: 4,
            sent_packets: Vec::new(),
        }
    }

    pub fn send(&mut self, packet: LiveAudioPacket) {
        self.sent_packets.push(packet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_packet_encoding_roundtrip() {
        let packet = LiveAudioPacket::NoteOn {
            channel: 1,
            note: 60,
            velocity: 100,
            frequency_hz: 261.63,
        };

        let framed = packet.encode_framed().expect("encode framed failed");
        assert_eq!(framed.last(), Some(&b'\n'));

        let decoded =
            LiveAudioPacket::decode_framed(&framed[..framed.len() - 1]).expect("decode failed");
        assert_eq!(packet, decoded);
    }

    #[test]
    fn test_mock_device_bridge_sends() {
        let mut bridge = MockDeviceBridge::default();
        assert!(bridge.connected);
        bridge.send(LiveAudioPacket::Ping);
        assert_eq!(bridge.sent_packets.len(), 1);
    }
}
