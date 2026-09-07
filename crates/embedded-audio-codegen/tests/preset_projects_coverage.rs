//! Coverage tests for all curated DAW presets and their codegen paths.

use embedded_audio_codegen::{generate_c_header, generate_embassy_code, generate_rust_song_code};

#[test]
fn all_preset_projects_generate_rust_embassy_and_c() {
    let presets = [
        embedded_audio_codegen::DawProject::default(),
        embedded_audio_codegen::DawProject::fm_cyberpunk(),
        embedded_audio_codegen::DawProject::boss_battle(),
        embedded_audio_codegen::DawProject::lofi_nostalgia(),
        embedded_audio_codegen::DawProject::sfx_showcase(),
    ];

    for project in &presets {
        let rust = generate_rust_song_code(project);
        assert!(rust.contains("pub const SONG_TITLE"));
        assert!(rust.contains("pub struct EmbeddedSongPlayer"));

        let embassy = generate_embassy_code(project);
        assert!(embassy.contains("use embassy_executor::task;"));

        let header = generate_c_header(project);
        assert!(header.contains("#define SONG_TITLE"));
    }
}
