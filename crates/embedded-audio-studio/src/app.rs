//! Main application state and responsive UI layout for embedded-audio-studio.

use eframe::egui::{self, Color32};
use embedded_audio_codegen::{DawProject, PinOutputMode, TargetMcu};
use std::sync::{Arc, Mutex};

use crate::audio_host::{AudioHostState, HostAudioDevice, VisualizerData};
use crate::device_link::{DeviceLinkState, render_device_link};
use crate::exporter::{ExporterState, render_exporter};
use crate::piano_roll::{PianoRollState, render_piano_roll};
use crate::synth_lab::{SynthLabState, render_synth_lab};
use crate::visualizers::render_visualizers;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ActiveTab {
    PianoRoll,
    SynthLab,
    HardwareVisualizers,
    DeviceLink,
    Exporter,
}

pub struct EmbeddedAudioStudioApp {
    pub active_tab: ActiveTab,
    pub project: DawProject,
    pub _audio_device: HostAudioDevice,
    pub audio_state: Arc<Mutex<AudioHostState>>,
    pub visualizer_data: Arc<Mutex<VisualizerData>>,
    pub piano_roll_state: PianoRollState,
    pub synth_lab_state: SynthLabState,
    pub device_link_state: DeviceLinkState,
    pub exporter_state: ExporterState,
    pub selected_preset_idx: usize,
}

impl EmbeddedAudioStudioApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let project = DawProject::chiptune_odyssey();
        let (audio_device, visualizer_data) = HostAudioDevice::new(project.clone());

        let audio_state = Arc::clone(&audio_device.state);
        let mut piano_roll_state = PianoRollState::default();
        piano_roll_state.auto_fit_all_tracks(&project.tracks);

        Self {
            active_tab: ActiveTab::PianoRoll,
            project,
            _audio_device: audio_device,
            audio_state,
            visualizer_data,
            piano_roll_state,
            synth_lab_state: SynthLabState::default(),
            device_link_state: DeviceLinkState::default(),
            exporter_state: ExporterState::default(),
            selected_preset_idx: 0,
        }
    }

    pub fn load_preset(&mut self, idx: usize) {
        self.selected_preset_idx = idx;
        let new_project = match idx {
            0 => DawProject::chiptune_odyssey(),
            1 => DawProject::fm_cyberpunk(),
            2 => DawProject::boss_battle(),
            3 => DawProject::lofi_nostalgia(),
            4 => DawProject::sfx_showcase(),
            _ => DawProject::chiptune_odyssey(),
        };

        self.project = new_project.clone();
        self.piano_roll_state.selected_track_idx = 0;
        self.synth_lab_state.selected_inst_idx = 0;
        self.piano_roll_state
            .auto_fit_all_tracks(&self.project.tracks);

        if let Ok(mut host) = self.audio_state.lock() {
            host.stop();
            host.set_project(new_project);
        }
    }
}

impl eframe::App for EmbeddedAudioStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle synth lab note preview triggers
        if let Some((inst_idx, midi_note)) = self.synth_lab_state.preview_note_trigger.take() {
            if let Ok(mut host) = self.audio_state.lock() {
                host.preview_note(inst_idx, midi_note, 110);
            }
        }

        // Handle SFX soundboard triggers
        if let Some(sfx_id) = self.synth_lab_state.preview_sfx_trigger.take() {
            if let Ok(mut host) = self.audio_state.lock() {
                host.preview_sfx(sfx_id);
            }
        }

        // Top Transport & Navigation Panel (Clean 2-tier responsive layout)
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(3.0);

            // Tier 1: App Brand, Transport Controls, Tempo, Hardware Profile & Presets
            ui.horizontal_wrapped(|ui| {
                ui.heading(
                    egui::RichText::new("⚡ EMBEDDED AUDIO STUDIO")
                        .color(Color32::from_rgb(100, 200, 255))
                        .strong(),
                );
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                let is_playing = if let Ok(host) = self.audio_state.lock() {
                    host.is_playing
                } else {
                    false
                };

                // Play / Pause Button
                let play_btn = if is_playing {
                    egui::Button::new("⏸ Pause").fill(Color32::from_rgb(180, 140, 40))
                } else {
                    egui::Button::new("▶ Play").fill(Color32::from_rgb(40, 140, 70))
                };

                if ui.add(play_btn).clicked() {
                    if let Ok(mut host) = self.audio_state.lock() {
                        if host.is_playing {
                            host.pause();
                        } else {
                            // Sync project state with host before starting playback
                            host.set_project(self.project.clone());
                            host.play();
                        }
                    }
                }

                // Stop Button
                if ui.button("⏹ Stop").clicked() {
                    if let Ok(mut host) = self.audio_state.lock() {
                        host.stop();
                    }
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // BPM Slider
                ui.label("BPM:");
                let mut bpm = self.project.bpm;
                if ui.add(egui::Slider::new(&mut bpm, 40..=240)).changed() {
                    self.project.bpm = bpm;
                    if let Ok(mut host) = self.audio_state.lock() {
                        host.set_bpm(bpm);
                    }
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Target MCU Selector
                let mcu_label = match self.project.hardware_target.target_mcu {
                    TargetMcu::Stm32U5 => "STM32U5 (160MHz)",
                    TargetMcu::Stm32F4 => "STM32F4 (168MHz)",
                    TargetMcu::Rp2040 => "RP2040 (125MHz)",
                    TargetMcu::Esp32RiscV => "ESP32-C3 (RISC-V)",
                    TargetMcu::GenericCortexM0 => "Cortex-M0+ (48MHz)",
                };
                egui::ComboBox::from_id_salt("target_mcu_combo")
                    .selected_text(format!("🎯 MCU: {}", mcu_label))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.project.hardware_target.target_mcu,
                            TargetMcu::Stm32U5,
                            "STM32U5 (Cortex-M33 @ 160MHz)",
                        );
                        ui.selectable_value(
                            &mut self.project.hardware_target.target_mcu,
                            TargetMcu::Stm32F4,
                            "STM32F4 (Cortex-M4 @ 168MHz)",
                        );
                        ui.selectable_value(
                            &mut self.project.hardware_target.target_mcu,
                            TargetMcu::Rp2040,
                            "RP2040 (Dual M0+ @ 125MHz)",
                        );
                        ui.selectable_value(
                            &mut self.project.hardware_target.target_mcu,
                            TargetMcu::Esp32RiscV,
                            "ESP32-C3 (RISC-V @ 160MHz)",
                        );
                        ui.selectable_value(
                            &mut self.project.hardware_target.target_mcu,
                            TargetMcu::GenericCortexM0,
                            "Generic Cortex-M0+ (@ 48MHz)",
                        );
                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Transducer Output Mode
                let out_label = match self.project.hardware_target.output_mode {
                    PinOutputMode::UltrasonicPwmSigmaDelta => "1-Bit Ultrasonic PWM ΣΔ",
                    PinOutputMode::PushPullComplementaryPwm => "Push-Pull PWM (6.6Vpp)",
                    PinOutputMode::FastArpeggioBeeper => "Beeper Tracker TDM",
                    PinOutputMode::DacOrI2sDirect => "DAC / I2S Direct",
                };
                egui::ComboBox::from_id_salt("output_pin_mode_combo")
                    .selected_text(format!("🔊 Output: {}", out_label))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.project.hardware_target.output_mode,
                            PinOutputMode::UltrasonicPwmSigmaDelta,
                            "1-Bit Ultrasonic PWM ΣΔ (Polyphonic + Speech)",
                        );
                        ui.selectable_value(
                            &mut self.project.hardware_target.output_mode,
                            PinOutputMode::PushPullComplementaryPwm,
                            "Push-Pull Complementary PWM (6.6V P-P Boost)",
                        );
                        ui.selectable_value(
                            &mut self.project.hardware_target.output_mode,
                            PinOutputMode::FastArpeggioBeeper,
                            "Fast-Arpeggio Beeper Engine (1-Bit Tracker)",
                        );
                        ui.selectable_value(
                            &mut self.project.hardware_target.output_mode,
                            PinOutputMode::DacOrI2sDirect,
                            "Direct 12-bit DAC / I2S Codec",
                        );
                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Demo Songs & SFX Selector
                let preset_names = [
                    "Chiptune Odyssey",
                    "Cyberpunk FM",
                    "Boss Battle 160BPM",
                    "Lo-Fi Nostalgia",
                    "🕹 SFX Showcase Pack",
                ];
                let current_label = preset_names
                    .get(self.selected_preset_idx)
                    .unwrap_or(&"Custom");

                let mut load_idx = None;
                egui::ComboBox::from_id_salt("preset_demo_dropdown")
                    .selected_text(format!("🎵 Demo: {}", current_label))
                    .show_ui(ui, |ui| {
                        for (i, name) in preset_names.iter().enumerate() {
                            if ui
                                .selectable_label(self.selected_preset_idx == i, *name)
                                .clicked()
                            {
                                load_idx = Some(i);
                            }
                        }
                    });

                if let Some(i) = load_idx {
                    self.load_preset(i);
                }
            });

            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);

            // Tier 2: Tabs and Project Metadata
            ui.horizontal(|ui| {
                // Tab Selection Buttons
                let tabs = [
                    (ActiveTab::PianoRoll, "🎹 Piano Roll & Sequencer"),
                    (ActiveTab::SynthLab, "🎛 Synth Lab & SFX"),
                    (
                        ActiveTab::HardwareVisualizers,
                        "📊 Hardware Visualizers & Piezo EQ",
                    ),
                    (ActiveTab::DeviceLink, "🔗 Device Link"),
                    (ActiveTab::Exporter, "💾 Exporter"),
                ];

                for (tab, label) in tabs {
                    let is_active = self.active_tab == tab;
                    let btn =
                        egui::Button::new(egui::RichText::new(label).strong()).fill(if is_active {
                            Color32::from_rgb(45, 90, 150)
                        } else {
                            Color32::from_rgb(22, 26, 36)
                        });
                    if ui.add(btn).clicked() {
                        self.active_tab = tab;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let eq_str = if self.project.hardware_target.piezo_eq.enabled {
                        "🔊 Piezo EQ: ON"
                    } else {
                        "🔊 Piezo EQ: OFF"
                    };
                    ui.label(
                        egui::RichText::new(eq_str)
                            .color(if self.project.hardware_target.piezo_eq.enabled {
                                Color32::GREEN
                            } else {
                                Color32::GRAY
                            })
                            .small(),
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "Song: \"{}\" by {}",
                            self.project.title, self.project.author
                        ))
                        .italics()
                        .color(Color32::from_rgb(170, 190, 220)),
                    );
                });
            });
            ui.add_space(3.0);
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let vis = self.visualizer_data.lock().unwrap().clone();
                ui.label(format!(
                    "Step: {} / {}",
                    vis.current_step + 1,
                    self.project.total_steps
                ));
                ui.separator();
                ui.label(format!("Tracks: {}", self.project.tracks.len()));
                ui.separator();
                ui.label(format!("Patches: {}", self.project.instruments.len()));
                ui.separator();
                ui.label(format!("Peak Level: {:.1} dB", vis.peak_db));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mcu_info = match self.project.hardware_target.target_mcu {
                        TargetMcu::Stm32U5 => "STM32U5 GPDMA PWM Carrier Mode",
                        TargetMcu::Stm32F4 => "STM32F4 DMA1/2 PWM Mode",
                        TargetMcu::Rp2040 => "RP2040 PIO/PWM Mode",
                        TargetMcu::Esp32RiscV => "ESP32-C3 LEDC/PWM Mode",
                        TargetMcu::GenericCortexM0 => "Generic Cortex-M0+ Mode",
                    };
                    ui.label(
                        egui::RichText::new(mcu_info)
                            .color(Color32::from_rgb(100, 180, 255))
                            .small(),
                    );
                });
            });
        });

        // Central Workspace Area
        egui::CentralPanel::default().show(ctx, |ui| {
            let vis = self.visualizer_data.lock().unwrap().clone();

            match self.active_tab {
                ActiveTab::PianoRoll => {
                    render_piano_roll(
                        ui,
                        &mut self.piano_roll_state,
                        &mut self.project.tracks,
                        self.project.total_steps,
                        self.project.steps_per_beat,
                        vis.current_step,
                    );
                }
                ActiveTab::SynthLab => {
                    render_synth_lab(
                        ui,
                        &mut self.synth_lab_state,
                        &mut self.project.instruments,
                        &mut self.project.hardware_target,
                    );
                }
                ActiveTab::HardwareVisualizers => {
                    render_visualizers(ui, &vis, &self.project);
                }
                ActiveTab::DeviceLink => {
                    render_device_link(ui, &mut self.device_link_state);
                }
                ActiveTab::Exporter => {
                    render_exporter(ui, &mut self.exporter_state, &self.project);
                }
            }
        });

        // Continuously sync project edits (track mutes, solo, volumes, note edits, patch configs) to audio engine in real time
        if let Ok(mut host) = self.audio_state.try_lock() {
            if host.project.tracks.len() == self.project.tracks.len() {
                for (i, track) in self.project.tracks.iter().enumerate() {
                    host.project.tracks[i].muted = track.muted;
                    host.project.tracks[i].solo = track.solo;
                    host.project.tracks[i].volume_q8 = track.volume_q8;
                    host.project.tracks[i].notes = track.notes.clone();
                }
            } else {
                host.project.tracks = self.project.tracks.clone();
            }
            host.project.instruments = self.project.instruments.clone();
            host.project.bpm = self.project.bpm;
            host.project.sample_rate_hz = self.project.sample_rate_hz;
            host.project.hardware_target = self.project.hardware_target;
        }

        // Request continuous repaint while playing for smooth animations
        let is_playing = if let Ok(host) = self.audio_state.lock() {
            host.is_playing
        } else {
            false
        };
        if is_playing || self.active_tab == ActiveTab::HardwareVisualizers {
            ctx.request_repaint();
        }
    }
}
