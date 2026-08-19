//! Real-time hardware emulation, STM32 / Piezo acoustic resonance compensator, DMA inspector, oscilloscope, and MCU profiler.

use crate::audio_host::VisualizerData;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};
use embedded_audio_codegen::{DawProject, HardwareTargetConfig, PinOutputMode, TargetMcu};

pub fn render_visualizers(ui: &mut egui::Ui, vis: &VisualizerData, project: &DawProject) {
    ui.vertical(|ui| {
        ui.heading("Microcontroller Hardware Emulation, Piezo Acoustic EQ & Telemetry");
        ui.add_space(4.0);

        ui.columns(2, |cols| {
            // Left Column: Oscilloscope, 1-Bit Sigma-Delta PWM Stream & DMA Double-Buffer
            cols[0].vertical(|ui| {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("PCM Audio Waveform (Oscilloscope)").strong());
                    ui.add_space(2.0);
                    render_scope(ui, &vis.pcm_scope, Color32::from_rgb(80, 200, 255));
                });

                ui.add_space(6.0);

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("1-Bit PWM / PDM Pulse Stream (Transducer Pin)").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mode_label = match project.hardware_target.output_mode {
                                PinOutputMode::UltrasonicPwmSigmaDelta => "500 kHz ΣΔ Carrier",
                                PinOutputMode::PushPullComplementaryPwm => "6.6V P-P Push-Pull",
                                PinOutputMode::FastArpeggioBeeper => "Beeper TDM Multiplex",
                                PinOutputMode::DacOrI2sDirect => "Direct 12-bit DAC",
                            };
                            ui.label(egui::RichText::new(mode_label).color(Color32::from_rgb(255, 180, 50)).small());
                        });
                    });
                    ui.label(egui::RichText::new("Physical mechanical inertia of the piezo disc demodulates this pulse train into continuous sound.").small());
                    ui.add_space(2.0);
                    render_pwm_stream(ui, &vis.pwm_pulse_stream, Color32::from_rgb(255, 140, 60));
                });

                ui.add_space(6.0);

                // STM32 / Cortex-M DMA Double Buffer Monitor
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Autonomous DMA Circular Double-Buffer Monitor").strong());
                    ui.add_space(3.0);
                    render_dma_buffer_monitor(ui, vis.current_step, &project.hardware_target);
                });
            });

            // Right Column: Piezo Acoustic Resonance Equalizer, Spectrum, and MCU Profiler
            cols[1].vertical(|ui| {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Piezo Acoustic Resonance Compensator (DSP EQ)").strong().color(Color32::from_rgb(100, 220, 255)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(if project.hardware_target.piezo_eq.enabled {
                                egui::RichText::new("EQ [ACTIVE]").color(Color32::GREEN).strong().small()
                            } else {
                                egui::RichText::new("BYPASS").color(Color32::GRAY).small()
                            });
                        });
                    });

                    ui.label(egui::RichText::new("Tames piercing 2-4 kHz piezo resonance spikes and boosts sub-harmonics.").small());
                    ui.add_space(2.0);
                    render_piezo_acoustic_curve(ui, &project.hardware_target.piezo_eq);
                });

                ui.add_space(6.0);

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("FFT Spectrum Analyzer").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("Peak: {:.1} dB", vis.peak_db));
                        });
                    });
                    ui.add_space(2.0);
                    render_spectrum(ui, &vis.spectrum_mags);
                });

                ui.add_space(6.0);

                ui.group(|ui| {
                    ui.label(egui::RichText::new("Embedded Hardware Resource Profiler").strong());
                    ui.add_space(4.0);
                    render_mcu_profiler(ui, project);
                });
            });
        });
    });
}

fn render_scope(ui: &mut egui::Ui, samples: &[f32], stroke_color: Color32) {
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 85.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_rgb(35, 42, 60)),
        StrokeKind::Inside,
    );

    let mid_y = rect.center().y;
    painter.line_segment(
        [Pos2::new(rect.min.x, mid_y), Pos2::new(rect.max.x, mid_y)],
        Stroke::new(1.0_f32, Color32::from_rgb(30, 38, 55)),
    );

    if samples.is_empty() {
        return;
    }

    let w = rect.width();
    let h = rect.height() * 0.45;
    let step_x = w / (samples.len() as f32);

    let mut prev_pt = Pos2::new(rect.min.x, mid_y - (samples[0] * h));
    for (i, &s) in samples.iter().enumerate().skip(1) {
        let x = rect.min.x + (i as f32 * step_x);
        let y = (mid_y - (s * h)).clamp(rect.min.y + 2.0, rect.max.y - 2.0);
        let curr_pt = Pos2::new(x, y);
        painter.line_segment([prev_pt, curr_pt], Stroke::new(1.5_f32, stroke_color));
        prev_pt = curr_pt;
    }
}

fn render_pwm_stream(ui: &mut egui::Ui, pulses: &[f32], pulse_color: Color32) {
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 85.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_rgb(35, 42, 60)),
        StrokeKind::Inside,
    );

    if pulses.is_empty() {
        return;
    }

    let num_display = 256.min(pulses.len());
    let step_x = rect.width() / (num_display as f32);
    let top_y = rect.min.y + 12.0;
    let bot_y = rect.max.y - 12.0;

    let start_idx = pulses.len().saturating_sub(num_display);
    let mut prev_pt = Pos2::new(
        rect.min.x,
        if pulses[start_idx] > 0.0 {
            top_y
        } else {
            bot_y
        },
    );

    for i in 0..num_display {
        let val = pulses[start_idx + i];
        let x = rect.min.x + (i as f32 * step_x);
        let target_y = if val > 0.0 { top_y } else { bot_y };

        let curr_pt = Pos2::new(x, target_y);
        painter.line_segment(
            [prev_pt, Pos2::new(x, prev_pt.y)],
            Stroke::new(1.2_f32, pulse_color),
        );
        painter.line_segment(
            [Pos2::new(x, prev_pt.y), curr_pt],
            Stroke::new(1.2_f32, pulse_color),
        );
        prev_pt = curr_pt;
    }
}

fn render_dma_buffer_monitor(ui: &mut egui::Ui, current_step: u32, hw: &HardwareTargetConfig) {
    let is_half_0 = (current_step % 2) == 0;
    ui.horizontal(|ui| {
        // Buffer 0
        let col_0 = if is_half_0 {
            Color32::from_rgb(40, 160, 90)
        } else {
            Color32::from_rgb(30, 45, 60)
        };
        let text_0 = if is_half_0 {
            "DMA Half 0: STREAMING ▶"
        } else {
            "DMA Half 0: READY"
        };
        ui.label(egui::RichText::new(text_0).color(col_0).monospace().small());

        ui.separator();

        // Buffer 1
        let col_1 = if !is_half_0 {
            Color32::from_rgb(220, 140, 30)
        } else {
            Color32::from_rgb(30, 45, 60)
        };
        let text_1 = if !is_half_0 {
            "DMA Half 1: DSP FILLING ⚡"
        } else {
            "DMA Half 1: READY"
        };
        ui.label(egui::RichText::new(text_1).color(col_1).monospace().small());

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} Words / Transfer", hw.dma_buffer_size)).small(),
            );
        });
    });
}

fn render_piezo_acoustic_curve(
    ui: &mut egui::Ui,
    eq: &embedded_audio_codegen::PiezoAcousticConfig,
) {
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 85.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_rgb(35, 42, 60)),
        StrokeKind::Inside,
    );

    // Center reference 0 dB line
    let mid_y = rect.min.y + 40.0;
    painter.line_segment(
        [Pos2::new(rect.min.x, mid_y), Pos2::new(rect.max.x, mid_y)],
        Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 65)),
    );

    let num_points = 64;
    let step_x = rect.width() / (num_points as f32);

    let mut prev_raw = Pos2::new(rect.min.x, mid_y);
    let mut prev_flat = Pos2::new(rect.min.x, mid_y);

    for i in 0..num_points {
        let frac = i as f32 / num_points as f32;
        let freq = 100.0 * (100.0_f32).powf(frac); // 100Hz to 10kHz log scale

        // Model raw piezo acoustic resonance peak at resonance_freq_hz
        let delta_f = (freq - eq.resonance_freq_hz) / 800.0;
        let raw_peak_db = 16.0 / (1.0 + delta_f * delta_f) - 6.0;

        let comp_db = if eq.enabled {
            let notch_db = eq.notch_depth_db / (1.0 + delta_f * delta_f);
            raw_peak_db + notch_db + (if freq < 800.0 { eq.bass_boost_db } else { 0.0 })
        } else {
            raw_peak_db
        };

        let x = rect.min.x + (i as f32 * step_x);
        let raw_y = (mid_y - raw_peak_db * 2.0).clamp(rect.min.y + 3.0, rect.max.y - 3.0);
        let flat_y = (mid_y - comp_db * 2.0).clamp(rect.min.y + 3.0, rect.max.y - 3.0);

        let pt_raw = Pos2::new(x, raw_y);
        let pt_flat = Pos2::new(x, flat_y);

        if i > 0 {
            // Draw Raw Piezo Spiky Curve in Coral Red
            painter.line_segment(
                [prev_raw, pt_raw],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 80, 80, 140)),
            );
            // Draw DSP Flattened Acoustic Curve in Neon Green
            painter.line_segment(
                [prev_flat, pt_flat],
                Stroke::new(1.8_f32, Color32::from_rgb(80, 240, 140)),
            );
        }

        prev_raw = pt_raw;
        prev_flat = pt_flat;
    }

    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 8.0),
        egui::Align2::LEFT_TOP,
        "Red: Raw Piezo Peak  |  Green: DSP Compensated",
        egui::FontId::monospace(10.0),
        Color32::from_rgb(180, 200, 220),
    );
}

fn render_spectrum(ui: &mut egui::Ui, mags: &[f32]) {
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 75.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_rgb(35, 42, 60)),
        StrokeKind::Inside,
    );

    if mags.is_empty() {
        return;
    }

    let num_bars = 48.min(mags.len());
    let bar_w = (rect.width() / num_bars as f32) - 1.5;

    for i in 0..num_bars {
        let mag = mags[i].clamp(0.0, 1.0);
        let bar_h = mag * (rect.height() - 10.0);
        let x = rect.min.x + (i as f32 * (bar_w + 1.5)) + 2.0;
        let y = rect.max.y - 5.0 - bar_h;

        let color = if mag > 0.8 {
            Color32::from_rgb(255, 90, 90)
        } else if mag > 0.4 {
            Color32::from_rgb(255, 200, 60)
        } else {
            Color32::from_rgb(90, 210, 140)
        };

        painter.rect_filled(
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(bar_w, bar_h.max(2.0))),
            1.5,
            color,
        );
    }
}

fn render_mcu_profiler(ui: &mut egui::Ui, project: &DawProject) {
    let mut total_notes = 0;
    for t in &project.tracks {
        total_notes += t.notes.len();
    }

    // Footprint estimations
    let bank_header_bytes = 16;
    let inst_bytes = project.instruments.len() * 24;
    let note_bytes = total_notes * 4;
    let total_rom_estimate = bank_header_bytes + inst_bytes + note_bytes;
    let ram_estimate_bytes =
        (project.tracks.len() * 32) + (project.hardware_target.dma_buffer_size * 2);

    ui.horizontal(|ui| {
        ui.label("Flash ROM Footprint:");
        ui.label(
            egui::RichText::new(format!(
                "{} B ({:.2} KB)",
                total_rom_estimate,
                total_rom_estimate as f32 / 1024.0
            ))
            .color(Color32::GREEN),
        );
        ui.separator();
        ui.label("RAM Usage:");
        ui.label(
            egui::RichText::new(format!("{} B", ram_estimate_bytes)).color(Color32::LIGHT_BLUE),
        );
    });

    ui.add_space(2.0);

    // Target MCU specific load
    let (target_name, mcu_mhz, mcu_scale) = match project.hardware_target.target_mcu {
        TargetMcu::Stm32U5 => ("STM32U5 (Cortex-M33)", 160, 0.22),
        TargetMcu::Stm32F4 => ("STM32F4 (Cortex-M4)", 168, 0.28),
        TargetMcu::Rp2040 => ("RP2040 (Cortex-M0+)", 125, 0.65),
        TargetMcu::Esp32RiscV => ("ESP32-C3 (RISC-V)", 160, 0.35),
        TargetMcu::GenericCortexM0 => ("Generic Cortex-M0+", 48, 1.8),
    };

    let est_load = (project.tracks.len() as f32 * mcu_scale).min(100.0);
    ui.horizontal(|ui| {
        ui.label(format!(
            "Target CPU Load ({} @ {}MHz):",
            target_name, mcu_mhz
        ));
        ui.label(
            egui::RichText::new(format!("{:.1}%", est_load))
                .color(Color32::from_rgb(100, 240, 150))
                .strong(),
        );
    });
}
