//! Theme styling for embedded-audio-studio.

use eframe::egui;

pub fn configure_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Dark cyberpunk/chiptune aesthetic
    style.visuals.dark_mode = true;
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(220, 230, 242));

    style.visuals.window_fill = egui::Color32::from_rgb(18, 20, 28);
    style.visuals.panel_fill = egui::Color32::from_rgb(14, 16, 22);

    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(22, 25, 36);
    style.visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(45, 52, 70));

    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(28, 33, 48);
    style.visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(50, 60, 85));

    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(42, 50, 75);
    style.visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(100, 160, 255));

    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 90, 150);
    style.visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(140, 200, 255));

    style.visuals.selection.bg_fill = egui::Color32::from_rgb(70, 120, 210);

    ctx.set_style(style);
}
