//! Code generation, export, and project definitions for embedded-audio.

pub mod c_gen;
pub mod embassy_gen;
pub mod project;
pub mod rust_gen;

pub use c_gen::generate_c_header;
pub use embassy_gen::generate_embassy_code;
pub use project::{
    AdsrConfig, DawProject, HardwareTargetConfig, Instrument, InstrumentKind, NoteEvent,
    PiezoAcousticConfig, PinOutputMode, TargetMcu, Track, WaveformType,
};
pub use rust_gen::generate_rust_song_code;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_project_rust_codegen() {
        let proj = DawProject::default();
        let code = generate_rust_song_code(&proj);
        assert!(code.contains("pub const SONG_TITLE: &str = \"Chiptune Odyssey\";"));
        assert!(code.contains("pub const SONG_BPM: u16 = 125;"));
        assert!(code.contains("pub struct EmbeddedSongPlayer"));
    }

    #[test]
    fn test_default_project_embassy_codegen() {
        let proj = DawProject::default();
        let code = generate_embassy_code(&proj);
        assert!(code.contains("use embassy_executor::task;"));
        assert!(code.contains("pub async fn embassy_audio_task"));
        assert!(code.contains("pub static SFX_CHANNEL"));
    }

    #[test]
    fn test_default_project_c_header_codegen() {
        let proj = DawProject::default();
        let header = generate_c_header(&proj);
        assert!(header.contains("#define SONG_TITLE \"Chiptune Odyssey\""));
        assert!(header.contains("#define SONG_BPM 125"));
        assert!(header.contains("typedef struct"));
    }
}
