//! Interactive sound lab, synth patch designer, embedded sound effects soundboard, and Piezo Acoustic Compensator.

use eframe::egui::{self, Color32, Pos2, Stroke, StrokeKind, Vec2};
use embedded_audio_codegen::{
    AdsrConfig, HardwareTargetConfig, Instrument, InstrumentKind, WaveformType,
};

pub struct SynthLabState {
    pub selected_inst_idx: usize,
    pub preview_note_trigger: Option<(usize, u8)>, // (instrument_idx, midi_note)
    pub preview_sfx_trigger: Option<usize>,        // sfx_id (0..7)
    pub selected_sfx_info: usize,
}

impl Default for SynthLabState {
    fn default() -> Self {
        Self {
            selected_inst_idx: 0,
            preview_note_trigger: None,
            preview_sfx_trigger: None,
            selected_sfx_info: 0,
        }
    }
}

pub fn render_synth_lab(
    ui: &mut egui::Ui,
    state: &mut SynthLabState,
    instruments: &mut [Instrument],
    hw_target: &mut HardwareTargetConfig,
) {
    ui.vertical(|ui| {
        ui.heading("Synth Lab, Sound Designer, Piezo EQ & SFX Soundboard");
        ui.add_space(4.0);

        // Section 1: Embedded SFX Soundboard (Interactive Sound Effects)
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🕹 Embedded Sound Effects Soundboard:").strong().color(Color32::from_rgb(255, 215, 80)));
                ui.label("(Click any button to trigger the SFX live)");
            });
            ui.add_space(4.0);

            let sfx_list = [
                (0, "🔊 Bootup Jingle", Color32::from_rgb(45, 110, 200)),
                (1, "🔫 Laser Pew-Pew", Color32::from_rgb(200, 70, 70)),
                (2, "🔌 Shutdown Power-Down", Color32::from_rgb(130, 80, 180)),
                (3, "🪙 Coin Pickup", Color32::from_rgb(220, 170, 30)),
                (4, "⭐ Power-Up Fanfare", Color32::from_rgb(40, 170, 110)),
                (5, "💥 Explosion Blast", Color32::from_rgb(210, 90, 40)),
                (6, "⚠️ Error Buzz Alert", Color32::from_rgb(220, 50, 80)),
                (7, "🦘 Jump / Spring", Color32::from_rgb(50, 160, 180)),
            ];

            ui.horizontal_wrapped(|ui| {
                for &(id, label, color) in &sfx_list {
                    let is_info_sel = state.selected_sfx_info == id;
                    let btn = egui::Button::new(egui::RichText::new(label).strong())
                        .fill(if is_info_sel { color } else { color.gamma_multiply(0.6) });

                    if ui.add(btn).clicked() {
                        state.preview_sfx_trigger = Some(id);
                        state.selected_sfx_info = id;
                    }
                    ui.add_space(4.0);
                }
            });

            ui.add_space(3.0);

            // SFX Embedded Code Recipe & Info
            let (recipe_title, recipe_code) = match state.selected_sfx_info {
                0 => ("System Bootup Chime", "engine.play_tone(72, AdsrSpec::new(2, 40, 180, 120)); // C5 -> E5 -> G5 -> C6 chime"),
                1 => ("Laser Blaster Pew-Pew", "engine.play_tone(100, AdsrSpec::click()); // Fast pitch-drop square chirp (1200Hz -> 200Hz)"),
                2 => ("Shutdown Power-Down", "engine.play_tone(48, AdsrSpec::new(5, 250, 0, 200)); // Sawtooth pitch droop"),
                3 => ("Coin Collect Pickup", "engine.play_tone(83, AdsrSpec::click()); // B5 -> E6 rapid 2-tone arpeggio"),
                4 => ("Power-Up Fanfare", "engine.play_fm(65, 200, 120, AdsrSpec::pad()); // Triumphant FM harmonic fanfare"),
                5 => ("Explosion Blast", "engine.play_noise(AdsrSpec::new(1, 160, 0, 80)); // 16-bit LFSR noise + sub bass thud"),
                6 => ("Error Alert Buzz", "engine.play_tone(48, AdsrSpec::new(1, 80, 200, 40)); // 120Hz square dual pulse alert"),
                7 => ("Jump / Spring", "engine.play_tone(72, AdsrSpec::new(2, 100, 100, 50)); // Triangle wave upward bounce"),
                _ => ("", ""),
            };

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Active SFX: {}", recipe_title)).color(Color32::from_rgb(120, 210, 255)));
                ui.label(egui::RichText::new(format!("MCU Code: {}", recipe_code)).monospace().small());
            });
        });

        ui.add_space(6.0);

        // Section 2: Transducer & Piezo Acoustic Resonance Equalizer
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🔊 Transducer & Piezo Acoustic Resonance Compensator").strong().color(Color32::from_rgb(100, 220, 255)));
                ui.checkbox(&mut hw_target.piezo_eq.enabled, "Enable DSP Resonance Notch");
            });
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("Resonance Peak:");
                ui.add(egui::Slider::new(&mut hw_target.piezo_eq.resonance_freq_hz, 1500.0..=5000.0).suffix(" Hz"));

                ui.add_space(8.0);
                ui.label("Notch Depth:");
                ui.add(egui::Slider::new(&mut hw_target.piezo_eq.notch_depth_db, -24.0..=0.0).suffix(" dB"));

                ui.add_space(8.0);
                ui.label("Sub-Bass Boost:");
                ui.add(egui::Slider::new(&mut hw_target.piezo_eq.bass_boost_db, 0.0..=12.0).suffix(" dB"));

                ui.add_space(8.0);
                ui.label("PWM Carrier:");
                ui.add(egui::Slider::new(&mut hw_target.piezo_eq.carrier_freq_khz, 100..=1000).suffix(" kHz"));
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // Section 3: Instrument Patches & Synthesis Engine
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Synth Patches:").strong());
            for (idx, inst) in instruments.iter().enumerate() {
                let is_sel = idx == state.selected_inst_idx;
                let btn = egui::Button::new(&inst.name)
                    .fill(if is_sel { Color32::from_rgb(50, 110, 160) } else { Color32::from_rgb(25, 30, 42) });
                if ui.add(btn).clicked() {
                    state.selected_inst_idx = idx;
                }
            }
        });

        ui.add_space(4.0);

        if let Some(inst) = instruments.get_mut(state.selected_inst_idx) {
            ui.columns(2, |cols| {
                // Left Column: Synthesis Parameters
                cols[0].vertical(|ui| {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Synthesis Engine").strong());
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            ui.text_edit_singleline(&mut inst.name);
                        });

                        ui.add_space(8.0);

                        // Mode switcher
                        let mut current_mode = match inst.kind {
                            InstrumentKind::Tone { .. } => 0,
                            InstrumentKind::Fm { .. } => 1,
                            InstrumentKind::Wavetable { .. } => 2,
                            InstrumentKind::Noise => 3,
                            InstrumentKind::Sample { .. } => 4,
                        };

                        ui.horizontal(|ui| {
                            ui.label("Type:");
                            ui.selectable_value(&mut current_mode, 0, "Tone / Pulse");
                            ui.selectable_value(&mut current_mode, 1, "2-Op FM");
                            ui.selectable_value(&mut current_mode, 2, "Wavetable");
                            ui.selectable_value(&mut current_mode, 3, "Noise");
                        });

                        ui.separator();

                        match &mut inst.kind {
                            InstrumentKind::Tone { waveform, duty } => {
                                ui.label(egui::RichText::new("Waveform & Duty Cycle").color(Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    ui.selectable_value(waveform, WaveformType::Square, "Square (50%)");
                                    ui.selectable_value(waveform, WaveformType::Pulse25, "Pulse (25%)");
                                    ui.selectable_value(waveform, WaveformType::Pulse12_5, "Pulse (12.5%)");
                                    ui.selectable_value(waveform, WaveformType::Sawtooth, "Sawtooth");
                                    ui.selectable_value(waveform, WaveformType::Triangle, "Triangle");
                                });

                                ui.add_space(6.0);
                                ui.label("Pulse Duty (0..255):");
                                ui.add(egui::Slider::new(duty, 0..=255));
                            }
                            InstrumentKind::Fm { mod_ratio_x100, mod_index_x100, feedback_x100 } => {
                                ui.label(egui::RichText::new("FM Synthesis Parameters").color(Color32::LIGHT_BLUE));

                                let mut ratio = *mod_ratio_x100 as f32 / 100.0;
                                if ui.add(egui::Slider::new(&mut ratio, 0.25..=8.0).text("Modulator Ratio")).changed() {
                                    *mod_ratio_x100 = (ratio * 100.0) as u16;
                                }

                                let mut index = *mod_index_x100 as f32 / 100.0;
                                if ui.add(egui::Slider::new(&mut index, 0.0..=5.0).text("Modulation Index")).changed() {
                                    *mod_index_x100 = (index * 100.0) as u16;
                                }

                                let mut fb = *feedback_x100 as f32 / 100.0;
                                if ui.add(egui::Slider::new(&mut fb, 0.0..=1.0).text("Feedback")).changed() {
                                    *feedback_x100 = (fb * 100.0) as u16;
                                }
                            }
                            InstrumentKind::Wavetable { preset, .. } => {
                                ui.label(egui::RichText::new("Wavetable Table Presets").color(Color32::LIGHT_BLUE));
                                ui.horizontal(|ui| {
                                    ui.selectable_value(preset, 0, "Sine");
                                    ui.selectable_value(preset, 1, "Triangle");
                                    ui.selectable_value(preset, 2, "Saw");
                                    ui.selectable_value(preset, 3, "Square");
                                });
                            }
                            InstrumentKind::Noise => {
                                ui.label("Pitch-tuned Galois LFSR Noise generator (NES / Game Boy style).");
                                ui.label("Low notes generate deep explosions & bass kicks; high notes generate hi-hat sizzle.");
                            }
                            InstrumentKind::Sample { is_adpcm, sample_rate_hz, .. } => {
                                ui.label(format!("Sample playback: {} ({} Hz)", if *is_adpcm { "IMA ADPCM" } else { "PCM8" }, sample_rate_hz));
                            }
                        }
                    });
                });

                // Right Column: ADSR Envelope Shaper & Curve Visualizer
                cols[1].vertical(|ui| {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("ADSR Envelope Shaper").strong());
                        ui.add_space(4.0);

                        render_adsr_controls(ui, &mut inst.adsr);
                        ui.add_space(8.0);
                        render_adsr_curve_painter(ui, &inst.adsr);
                    });
                });
            });

            ui.add_space(10.0);

            // Audition keyboard bar
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Audition Keyboard:").strong());
                    ui.label("(Click to trigger note)");
                });
                ui.add_space(4.0);

                let notes = [
                    (60, "C4"), (61, "C#4"), (62, "D4"), (63, "D#4"), (64, "E4"),
                    (65, "F4"), (66, "F#4"), (67, "G4"), (68, "G#4"), (69, "A4"),
                    (70, "A#4"), (71, "B4"), (72, "C5"), (74, "D5"), (76, "E5"),
                ];

                ui.horizontal(|ui| {
                    for &(midi_note, label) in &notes {
                        let is_black = label.contains('#');
                        let btn = egui::Button::new(label)
                            .min_size(Vec2::new(34.0, 48.0))
                            .fill(if is_black { Color32::from_rgb(30, 35, 45) } else { Color32::from_rgb(210, 220, 235) });
                        let resp = ui.add(btn);
                        if resp.clicked() {
                            state.preview_note_trigger = Some((state.selected_inst_idx, midi_note));
                        }
                    }
                });
            });
        }
    });
}

fn render_adsr_controls(ui: &mut egui::Ui, adsr: &mut AdsrConfig) {
    ui.horizontal(|ui| {
        ui.label("Attack:");
        ui.add(egui::Slider::new(&mut adsr.attack_ms, 0..=500).suffix(" ms"));
    });

    ui.horizontal(|ui| {
        ui.label("Decay:");
        ui.add(egui::Slider::new(&mut adsr.decay_ms, 0..=1000).suffix(" ms"));
    });

    ui.horizontal(|ui| {
        ui.label("Sustain:");
        let mut sustain_pct = adsr.sustain_q8 as f32 / 2.55;
        if ui
            .add(egui::Slider::new(&mut sustain_pct, 0.0..=100.0).suffix(" %"))
            .changed()
        {
            adsr.sustain_q8 = (sustain_pct * 2.55) as u8;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Release:");
        ui.add(egui::Slider::new(&mut adsr.release_ms, 0..=1500).suffix(" ms"));
    });
}

fn render_adsr_curve_painter(ui: &mut egui::Ui, adsr: &AdsrConfig) {
    let (response, painter) = ui.allocate_painter(Vec2::new(260.0, 90.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 4.0, Color32::from_rgb(16, 20, 28));
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 65)),
        StrokeKind::Inside,
    );

    let total_time_ms = (adsr.attack_ms + adsr.decay_ms + 200 + adsr.release_ms).max(100) as f32;
    let w = rect.width() - 20.0;
    let h = rect.height() - 20.0;

    let p0 = Pos2::new(rect.min.x + 10.0, rect.max.y - 10.0);
    let p_a = Pos2::new(
        p0.x + (adsr.attack_ms as f32 / total_time_ms) * w,
        rect.min.y + 10.0,
    );
    let p_d = Pos2::new(
        p_a.x + (adsr.decay_ms as f32 / total_time_ms) * w,
        rect.max.y - 10.0 - (adsr.sustain_q8 as f32 / 255.0) * h,
    );
    let p_s = Pos2::new(p_d.x + (200.0 / total_time_ms) * w, p_d.y);
    let p_r = Pos2::new(
        p_s.x + (adsr.release_ms as f32 / total_time_ms) * w,
        rect.max.y - 10.0,
    );

    let stroke = Stroke::new(2.0_f32, Color32::from_rgb(100, 200, 255));
    painter.line_segment([p0, p_a], stroke);
    painter.line_segment([p_a, p_d], stroke);
    painter.line_segment([p_d, p_s], stroke);
    painter.line_segment([p_s, p_r], stroke);

    painter.circle_filled(p_a, 3.5, Color32::from_rgb(255, 180, 50));
    painter.circle_filled(p_d, 3.5, Color32::from_rgb(255, 180, 50));
    painter.circle_filled(p_s, 3.5, Color32::from_rgb(255, 180, 50));
    painter.circle_filled(p_r, 3.5, Color32::from_rgb(255, 180, 50));
}
