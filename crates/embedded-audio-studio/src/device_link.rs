//! Hardware-in-the-loop device connection and live audio streaming bridge.

use eframe::egui::{self, Color32};
use embedded_audio_live::MockDeviceBridge;

pub struct DeviceLinkState {
    pub is_connected: bool,
    pub transport_type: usize, // 0 = Loopback Mock, 1 = USB CDC, 2 = UART Serial
    pub port_name: String,
    pub baud_rate: u32,
    pub mock_bridge: MockDeviceBridge,
    pub last_telemetry: Option<(u8, u32, u32)>, // (cpu_pct, underruns, free_heap)
    pub log_messages: Vec<String>,
}

impl Default for DeviceLinkState {
    fn default() -> Self {
        Self {
            is_connected: false,
            transport_type: 0,
            port_name: "/dev/ttyACM0".to_string(),
            baud_rate: 115200,
            mock_bridge: MockDeviceBridge::new("RP2040 Embedded PWM Board"),
            last_telemetry: Some((12, 0, 18432)),
            log_messages: vec!["Ready to connect to embedded target.".to_string()],
        }
    }
}

pub fn render_device_link(ui: &mut egui::Ui, state: &mut DeviceLinkState) {
    ui.vertical(|ui| {
        ui.heading("Device Live-Link & Real-Time Hardware Auditioning");
        ui.add_space(4.0);

        ui.columns(2, |cols| {
            // Left: Connection Settings
            cols[0].group(|ui| {
                ui.label(egui::RichText::new("Target Hardware Interface").strong());
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Interface:");
                    ui.selectable_value(&mut state.transport_type, 0, "Mock Loopback");
                    ui.selectable_value(&mut state.transport_type, 1, "USB CDC");
                    ui.selectable_value(&mut state.transport_type, 2, "UART Serial");
                });

                if state.transport_type != 0 {
                    ui.horizontal(|ui| {
                        ui.label("Port:");
                        ui.text_edit_singleline(&mut state.port_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Baud Rate:");
                        ui.selectable_value(&mut state.baud_rate, 115200, "115200");
                        ui.selectable_value(&mut state.baud_rate, 921600, "921600 (High-Speed)");
                    });
                }

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if !state.is_connected {
                        if ui.button(egui::RichText::new("⚡ Connect to Target").color(Color32::GREEN)).clicked() {
                            state.is_connected = true;
                            state.log_messages.push(format!("Connected to target via {}!", if state.transport_type == 0 { "Mock Transport" } else { &state.port_name }));
                        }
                    } else {
                        if ui.button(egui::RichText::new("⏹ Disconnect").color(Color32::LIGHT_RED)).clicked() {
                            state.is_connected = false;
                            state.log_messages.push("Disconnected from target.".to_string());
                        }
                    }
                });
            });

            // Right: Target Telemetry & Logs
            cols[1].group(|ui| {
                ui.label(egui::RichText::new("Device Telemetry & Health").strong());
                ui.add_space(4.0);

                if state.is_connected {
                    ui.horizontal(|ui| {
                        ui.label("Status:");
                        ui.label(egui::RichText::new("ONLINE (STREAMING)").color(Color32::GREEN).strong());
                    });
                    ui.horizontal(|ui| {
                        ui.label("Device:");
                        ui.label(&state.mock_bridge.board_name);
                    });
                    if let Some((cpu, underruns, heap)) = state.last_telemetry {
                        ui.horizontal(|ui| {
                            ui.label("MCU CPU Load:");
                            ui.label(egui::RichText::new(format!("{}%", cpu)).color(Color32::YELLOW));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Buffer Underruns:");
                            ui.label(format!("{}", underruns));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Free RAM:");
                            ui.label(format!("{} bytes", heap));
                        });
                    }
                } else {
                    ui.label(egui::RichText::new("Target offline. Connect to stream notes and audio directly to MCU pins.").italics());
                }

                ui.add_space(6.0);
                ui.separator();
                ui.label(egui::RichText::new("Activity Log:").small());
                egui::ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
                    for msg in state.log_messages.iter().rev().take(6) {
                        ui.label(egui::RichText::new(msg).monospace().small());
                    }
                });
            });
        });
    });
}
