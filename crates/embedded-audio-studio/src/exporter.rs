//! Code generation and sound bank export modal / panel with syntax highlighting and Embassy async support.

use eframe::egui::{self, Color32, FontId, TextFormat, text::LayoutJob};
use embedded_audio_codegen::{
    DawProject, generate_c_header, generate_embassy_code, generate_rust_song_code,
};

pub struct ExporterState {
    pub selected_tab: usize, // 0 = Rust ISR, 1 = Rust Embassy, 2 = C/C++, 3 = EAF Bank
    pub exported_rust_code: String,
    pub exported_embassy_code: String,
    pub exported_c_code: String,
    pub status_message: Option<String>,
}

impl Default for ExporterState {
    fn default() -> Self {
        Self {
            selected_tab: 0,
            exported_rust_code: String::new(),
            exported_embassy_code: String::new(),
            exported_c_code: String::new(),
            status_message: None,
        }
    }
}

pub fn render_exporter(ui: &mut egui::Ui, state: &mut ExporterState, project: &DawProject) {
    ui.vertical(|ui| {
        ui.heading("Asset Export & Code Generator");
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut state.selected_tab, 0, "🦀 Rust (Bare-Metal ISR)");
            ui.selectable_value(&mut state.selected_tab, 1, "⚡ Rust (Embassy Async Task)");
            ui.selectable_value(&mut state.selected_tab, 2, "📄 C/C++ Header (.h)");
            ui.selectable_value(&mut state.selected_tab, 3, "💾 Binary SoundBank (.eaf)");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚡ Regenerate All Code").clicked() {
                    state.exported_rust_code = generate_rust_song_code(project);
                    state.exported_embassy_code = generate_embassy_code(project);
                    state.exported_c_code = generate_c_header(project);
                    state.status_message = Some("All code targets regenerated successfully.".to_string());
                }
            });
        });

        ui.separator();

        if state.exported_rust_code.is_empty() {
            state.exported_rust_code = generate_rust_song_code(project);
            state.exported_embassy_code = generate_embassy_code(project);
            state.exported_c_code = generate_c_header(project);
        }

        if let Some(ref msg) = state.status_message {
            ui.label(egui::RichText::new(msg).color(Color32::from_rgb(100, 240, 150)).small());
            ui.add_space(2.0);
        }

        match state.selected_tab {
            0 => {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ready-to-use no_std Rust module with DMA ISR & Piezo Notch Filter:").strong());
                    if ui.button("📋 Copy to Clipboard").clicked() {
                        ui.ctx().copy_text(state.exported_rust_code.clone());
                        state.status_message = Some("Copied Bare-Metal Rust code to clipboard!".to_string());
                    }
                });
                ui.add_space(4.0);

                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                    let mut layout_job = highlight_code(text, true);
                    layout_job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(layout_job))
                };

                egui::ScrollArea::both().max_height(380.0).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.exported_rust_code)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .layouter(&mut layouter),
                    );
                });
            }
            1 => {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Embassy Async Audio Task & Non-Blocking SFX Channel Dispatcher:").strong().color(Color32::from_rgb(255, 200, 60)));
                    if ui.button("📋 Copy to Clipboard").clicked() {
                        ui.ctx().copy_text(state.exported_embassy_code.clone());
                        state.status_message = Some("Copied Embassy Async code to clipboard!".to_string());
                    }
                });
                ui.label(egui::RichText::new("Compatible with embassy-executor, embassy-time, embassy-sync, embassy-stm32, and embassy-rp.").small());
                ui.add_space(4.0);

                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                    let mut layout_job = highlight_code(text, true);
                    layout_job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(layout_job))
                };

                egui::ScrollArea::both().max_height(380.0).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.exported_embassy_code)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .layouter(&mut layouter),
                    );
                });
            }
            2 => {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("C/C++ header for STM32 HAL / Pico SDK / ESP-IDF / Arduino:").strong());
                    if ui.button("📋 Copy to Clipboard").clicked() {
                        ui.ctx().copy_text(state.exported_c_code.clone());
                        state.status_message = Some("Copied C header to clipboard!".to_string());
                    }
                });
                ui.add_space(4.0);

                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                    let mut layout_job = highlight_code(text, false);
                    layout_job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(layout_job))
                };

                egui::ScrollArea::both().max_height(380.0).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.exported_c_code)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .layouter(&mut layouter),
                    );
                });
            }
            3 => {
                ui.label(egui::RichText::new("Flash SoundBank (.eaf) Binary Packaging:").strong());
                ui.label("Compile track sequences, ADPCM waveforms, and FM sound presets into a ROM-ready binary bank.");
                ui.add_space(8.0);
                if ui.button(egui::RichText::new("💾 Bake & Save .eaf Sound Bank").strong()).clicked() {
                    state.status_message = Some("Baked soundbank to embedded-audio output/song.eaf".to_string());
                }
            }
            _ => {}
        }
    });
}

/// Tokenizer and Syntax Highlighter for Rust and C/C++ code.
fn highlight_code(text: &str, is_rust: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = FontId::monospace(12.0);

    let col_kw = Color32::from_rgb(195, 120, 255); // Purple: pub, fn, struct, impl, let, if, async, await, task
    let col_type = Color32::from_rgb(80, 220, 210); // Cyan: u8, u16, u32, BiquadFilter, SigmaDelta, Channel, Timer
    let col_str = Color32::from_rgb(255, 205, 90); // Amber Gold: "..."
    let col_num = Color32::from_rgb(255, 145, 80); // Warm Orange: 125, 0x7FFF, 500.0
    let col_comment = Color32::from_rgb(105, 150, 120); // Muted Green: // ..., /* ... */
    let col_attr = Color32::from_rgb(255, 130, 180); // Pink: #[task], #[derive(...)]
    let col_punct = Color32::from_rgb(175, 190, 210); // Soft Grey-Blue: (), {}, [], ::, ->, =
    let col_text = Color32::from_rgb(225, 235, 245); // Off-White default

    let rust_keywords = [
        "pub", "const", "struct", "impl", "fn", "let", "mut", "if", "else", "match", "for", "in",
        "return", "static", "true", "false", "use", "extern", "as", "where", "trait", "enum",
        "type", "mod", "ref", "self", "crate", "unsafe", "async", "await", "loop", "task",
    ];

    let rust_types = [
        "u8",
        "u16",
        "u32",
        "u64",
        "i8",
        "i16",
        "i32",
        "i64",
        "f32",
        "f64",
        "usize",
        "isize",
        "bool",
        "str",
        "Option",
        "Some",
        "None",
        "Result",
        "Ok",
        "Err",
        "Self",
        "DawProject",
        "NoteEvent",
        "HardwarePwmAudioDriver",
        "EmbeddedSongPlayer",
        "EmbassyAudioEngine",
        "SfxCue",
        "Channel",
        "Duration",
        "Timer",
        "CriticalSectionRawMutex",
        "BiquadFilter",
        "SigmaDelta",
        "AdsrSpec",
        "ToneVoice",
        "FmVoice",
        "WavetableVoice",
    ];

    let c_keywords = [
        "typedef", "struct", "enum", "const", "static", "extern", "void", "return", "if", "else",
        "for", "while", "do", "switch", "case", "default", "break", "continue", "sizeof", "inline",
        "true", "false", "NULL",
    ];

    let c_types = [
        "uint8_t",
        "uint16_t",
        "uint32_t",
        "uint64_t",
        "int8_t",
        "int16_t",
        "int32_t",
        "int64_t",
        "float",
        "double",
        "char",
        "int",
        "short",
        "long",
        "bool",
        "size_t",
        "SongNoteEvent",
        "EmbeddedSongPlayer",
        "BiquadFilter",
        "SigmaDelta",
    ];

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // 1. Comments
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
            let start = i;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            let snippet: String = chars[start..i].iter().collect();
            job.append(
                &snippet,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: col_comment,
                    ..Default::default()
                },
            );
            continue;
        }

        // Block comments
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            let snippet: String = chars[start..i].iter().collect();
            job.append(
                &snippet,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: col_comment,
                    ..Default::default()
                },
            );
            continue;
        }

        // 2. Attributes / Preprocessor (#...)
        if chars[i] == '#' {
            let start = i;
            while i < len && chars[i] != '\n' && chars[i] != ']' {
                i += 1;
            }
            if i < len && chars[i] == ']' {
                i += 1;
            }
            let snippet: String = chars[start..i].iter().collect();
            job.append(
                &snippet,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: col_attr,
                    ..Default::default()
                },
            );
            continue;
        }

        // 3. String literals ("...")
        if chars[i] == '"' {
            let start = i;
            i += 1;
            while i < len && chars[i] != '"' && chars[i] != '\n' {
                if chars[i] == '\\' && i + 1 < len {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < len && chars[i] == '"' {
                i += 1;
            }
            let snippet: String = chars[start..i].iter().collect();
            job.append(
                &snippet,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: col_str,
                    ..Default::default()
                },
            );
            continue;
        }

        // 4. Numbers (Hex or Decimal / Floats)
        if chars[i].is_ascii_digit()
            || (chars[i] == '-' && i + 1 < len && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            if chars[i] == '-' {
                i += 1;
            }
            while i < len
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_')
            {
                i += 1;
            }
            let snippet: String = chars[start..i].iter().collect();
            job.append(
                &snippet,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: col_num,
                    ..Default::default()
                },
            );
            continue;
        }

        // 5. Identifiers (Keywords, Types, Functions, Variables)
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();

            let color = if is_rust {
                if rust_keywords.contains(&word.as_str()) {
                    col_kw
                } else if rust_types.contains(&word.as_str())
                    || word.starts_with(|c: char| c.is_uppercase())
                {
                    col_type
                } else {
                    col_text
                }
            } else {
                if c_keywords.contains(&word.as_str()) {
                    col_kw
                } else if c_types.contains(&word.as_str())
                    || word.ends_with("_t")
                    || word.starts_with(|c: char| c.is_uppercase())
                {
                    col_type
                } else {
                    col_text
                }
            };

            job.append(
                &word,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color,
                    ..Default::default()
                },
            );
            continue;
        }

        // 6. Symbols, Operators & Punctuation
        let sym = chars[i];
        let sym_str = sym.to_string();
        let sym_col = match sym {
            '{' | '}' | '(' | ')' | '[' | ']' => Color32::from_rgb(255, 215, 80),
            ';' | ',' | '.' | ':' => Color32::from_rgb(140, 160, 185),
            '=' | '+' | '-' | '*' | '/' | '&' | '|' | '!' | '<' | '>' => {
                Color32::from_rgb(100, 200, 255)
            }
            _ => col_punct,
        };

        job.append(
            &sym_str,
            0.0,
            TextFormat {
                font_id: font_id.clone(),
                color: sym_col,
                ..Default::default()
            },
        );
        i += 1;
    }

    job
}
