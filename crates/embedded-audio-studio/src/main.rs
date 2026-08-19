//! Embedded Audio Studio - Desktop DAW & Chiptune Lab for Microcontrollers

mod app;
mod audio_host;
mod device_link;
mod exporter;
mod piano_roll;
mod synth_lab;
mod theme;
mod visualizers;

use app::EmbeddedAudioStudioApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("Embedded Audio Studio - Lo-Fi & Chiptune DAW"),
        ..Default::default()
    };

    eframe::run_native(
        "embedded-audio-studio",
        native_options,
        Box::new(|cc| {
            theme::configure_theme(&cc.egui_ctx);
            Ok(Box::new(EmbeddedAudioStudioApp::new(cc)))
        }),
    )
}
