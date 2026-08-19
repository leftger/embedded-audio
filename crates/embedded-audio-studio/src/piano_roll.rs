//! Modern High-Contrast Step-Sequencer and Piano-Roll timeline editor with full-range multi-octave visibility.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};
use embedded_audio_codegen::{NoteEvent, Track};

/// Distinct high-contrast palette for tracks.
pub const TRACK_COLORS: [Color32; 5] = [
    Color32::from_rgb(60, 170, 255),  // Electric Blue (Lead)
    Color32::from_rgb(60, 230, 140),  // Emerald Green (Bass)
    Color32::from_rgb(255, 165, 45),  // Amber Gold (Percussion/SFX)
    Color32::from_rgb(205, 100, 255), // Neon Purple (FM/Wavetable)
    Color32::from_rgb(255, 85, 150),  // Hot Pink (Arp)
];

pub struct PianoRollState {
    pub selected_track_idx: usize,
    pub base_midi_note: u8,
    pub num_notes_displayed: usize,
    pub current_velocity: u8,
    pub step_width: f32,
    pub key_height: f32,
    pub show_ghost_notes: bool,
}

impl Default for PianoRollState {
    fn default() -> Self {
        Self {
            selected_track_idx: 0,
            base_midi_note: 24,      // C1 (covers sub-bass and explosions)
            num_notes_displayed: 72, // 6 full octaves: C1 to B6 (covers all bass, leads, and hi-hats!)
            current_velocity: 100,
            step_width: 32.0,
            key_height: 18.0,
            show_ghost_notes: true,
        }
    }
}

impl PianoRollState {
    /// Center the view on the pitch range of the given track.
    pub fn auto_fit_octave(&mut self, track: &Track) {
        if track.notes.is_empty() {
            return;
        }
        let min_n = track.notes.iter().map(|n| n.note).min().unwrap_or(24);
        let max_n = track.notes.iter().map(|n| n.note).max().unwrap_or(95);

        // Set base note so all notes are visible
        let target_base = (min_n / 12 * 12).saturating_sub(12).clamp(12, 60);
        self.base_midi_note = target_base;
        let needed_range = (max_n as usize + 12)
            .saturating_sub(target_base as usize)
            .max(48);
        self.num_notes_displayed = needed_range.min(84);
    }

    /// Fit all notes across all tracks in the project.
    pub fn auto_fit_all_tracks(&mut self, tracks: &[Track]) {
        let mut min_n = 127;
        let mut max_n = 0;
        let mut has_notes = false;

        for t in tracks {
            for n in &t.notes {
                has_notes = true;
                if n.note < min_n {
                    min_n = n.note;
                }
                if n.note > max_n {
                    max_n = n.note;
                }
            }
        }

        if has_notes {
            self.base_midi_note = (min_n / 12 * 12).clamp(12, 60);
            let needed = (max_n as usize + 12)
                .saturating_sub(self.base_midi_note as usize)
                .max(48);
            self.num_notes_displayed = needed.min(84);
        } else {
            self.base_midi_note = 24;
            self.num_notes_displayed = 72;
        }
    }
}

pub fn render_piano_roll(
    ui: &mut egui::Ui,
    state: &mut PianoRollState,
    tracks: &mut [Track],
    total_steps: u32,
    steps_per_beat: u32,
    current_playing_step: u32,
) {
    ui.vertical(|ui| {
        // Track selector tabs & track settings
        let mut solo_clicked_idx = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Tracks:").strong());
            for (idx, track) in tracks.iter_mut().enumerate() {
                let is_selected = idx == state.selected_track_idx;
                let track_color = TRACK_COLORS[idx % TRACK_COLORS.len()];

                let btn_text = egui::RichText::new(format!("● {}", track.name))
                    .color(if is_selected {
                        Color32::WHITE
                    } else if track.muted {
                        Color32::GRAY
                    } else {
                        track_color
                    })
                    .strong();

                let btn = egui::Button::new(btn_text).fill(if is_selected {
                    track_color.gamma_multiply(0.6)
                } else {
                    Color32::from_rgb(25, 30, 42)
                });

                if ui.add(btn).clicked() {
                    state.selected_track_idx = idx;
                }

                // Mute button (M)
                let m_text = egui::RichText::new(if track.muted { "MUTE" } else { "M" })
                    .color(if track.muted {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(160, 170, 185)
                    })
                    .small()
                    .strong();
                let m_btn = egui::Button::new(m_text).fill(if track.muted {
                    Color32::from_rgb(220, 50, 50)
                } else {
                    Color32::from_rgb(32, 38, 50)
                });
                if ui.add(m_btn).clicked() {
                    track.muted = !track.muted;
                }

                // Solo button (S) - Standard exclusive solo (or Shift-click for multi-solo)
                let s_text = egui::RichText::new(if track.solo { "SOLO" } else { "S" })
                    .color(if track.solo {
                        Color32::BLACK
                    } else {
                        Color32::from_rgb(160, 170, 185)
                    })
                    .small()
                    .strong();
                let s_btn = egui::Button::new(s_text).fill(if track.solo {
                    Color32::from_rgb(240, 200, 40)
                } else {
                    Color32::from_rgb(32, 38, 50)
                });
                if ui.add(s_btn).clicked() {
                    solo_clicked_idx = Some(idx);
                }

                ui.add_space(8.0);
            }
        });

        // Apply exclusive solo logic
        if let Some(s_idx) = solo_clicked_idx {
            let was_solo = tracks.get(s_idx).map(|t| t.solo).unwrap_or(false);
            let shift_held = ui.input(|i| i.modifiers.shift || i.modifiers.ctrl);
            if was_solo {
                if let Some(t) = tracks.get_mut(s_idx) {
                    t.solo = false;
                }
            } else if shift_held {
                if let Some(t) = tracks.get_mut(s_idx) {
                    t.solo = true;
                }
            } else {
                for (i, t) in tracks.iter_mut().enumerate() {
                    t.solo = i == s_idx;
                }
            }
        }

        ui.add_space(4.0);

        // Track Details Toolbar & Range Zoom Controls
        let active_track_color = TRACK_COLORS[state.selected_track_idx % TRACK_COLORS.len()];
        let mut do_fit_all = false;
        let mut toolbar_solo_toggled = false;
        if let Some(track) = tracks.get_mut(state.selected_track_idx) {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!("Track: {}", track.name))
                        .color(active_track_color)
                        .strong(),
                );
                ui.add_space(6.0);

                // Quick Mute & Solo toggle for active track
                let is_muted = track.muted;
                ui.checkbox(
                    &mut track.muted,
                    egui::RichText::new("Mute").color(if is_muted {
                        Color32::from_rgb(255, 90, 90)
                    } else {
                        Color32::WHITE
                    }),
                );
                let is_solo = track.solo;
                let mut new_solo = is_solo;
                if ui
                    .checkbox(
                        &mut new_solo,
                        egui::RichText::new("Solo").color(if is_solo {
                            Color32::from_rgb(255, 220, 60)
                        } else {
                            Color32::WHITE
                        }),
                    )
                    .changed()
                {
                    toolbar_solo_toggled = true;
                }

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                ui.label("Vol:");
                let mut vol = track.volume_q8 as f32 / 2.55;
                if ui
                    .add(egui::Slider::new(&mut vol, 0.0..=100.0).suffix("%"))
                    .changed()
                {
                    track.volume_q8 = (vol * 2.55) as u8;
                }

                ui.add_space(6.0);
                ui.label("Vel:");
                ui.add(egui::Slider::new(&mut state.current_velocity, 1..=127));

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // Octave & Range View Presets
                ui.label(egui::RichText::new("Range:").strong());
                let min_note_name = format!("C{}", state.base_midi_note / 12 - 1);
                let max_note_name = format!(
                    "B{}",
                    (state.base_midi_note as usize + state.num_notes_displayed) / 12 - 1
                );
                ui.label(
                    egui::RichText::new(format!("{} - {}", min_note_name, max_note_name))
                        .strong()
                        .color(Color32::from_rgb(255, 230, 100)),
                );

                let mut do_fit_track = false;

                if ui.button("🔍 Fit All").clicked() {
                    do_fit_all = true;
                }

                if ui.button("🎯 Fit Track").clicked() {
                    do_fit_track = true;
                }

                if ui.button("▲ Shift Up").clicked() && state.base_midi_note < 60 {
                    state.base_midi_note += 12;
                }
                if ui.button("▼ Shift Down").clicked() && state.base_midi_note >= 12 {
                    state.base_midi_note -= 12;
                }

                // Vertical Zoom controls
                if ui.button("➕ Zoom Y").clicked() && state.key_height < 32.0 {
                    state.key_height += 2.0;
                }
                if ui.button("➖ Zoom Y").clicked() && state.key_height > 12.0 {
                    state.key_height -= 2.0;
                }

                ui.checkbox(&mut state.show_ghost_notes, "Ghost Tracks");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🗑 Clear Track").clicked() {
                        track.notes.clear();
                    }
                    ui.label(format!("Notes: {}", track.notes.len()));
                });

                if do_fit_track {
                    state.auto_fit_octave(track);
                }
            });
        }

        if do_fit_all {
            state.auto_fit_all_tracks(tracks);
        }

        if toolbar_solo_toggled {
            let s_idx = state.selected_track_idx;
            let was_solo = tracks.get(s_idx).map(|t| t.solo).unwrap_or(false);
            if was_solo {
                if let Some(t) = tracks.get_mut(s_idx) {
                    t.solo = false;
                }
            } else {
                for (i, t) in tracks.iter_mut().enumerate() {
                    t.solo = i == s_idx;
                }
            }
        }

        ui.add_space(4.0);

        // Check if any notes in the selected track fall outside visible bounds
        let base_midi_note = state.base_midi_note;
        let num_notes_displayed = state.num_notes_displayed;
        let top_midi_note = base_midi_note as usize + num_notes_displayed;

        if let Some(track) = tracks.get(state.selected_track_idx) {
            let notes_above = track
                .notes
                .iter()
                .filter(|n| n.note as usize >= top_midi_note)
                .count();
            let notes_below = track
                .notes
                .iter()
                .filter(|n| (n.note as usize) < base_midi_note as usize)
                .count();

            if notes_above > 0 || notes_below > 0 {
                ui.horizontal(|ui| {
                    if notes_above > 0 {
                        if ui
                            .button(
                                egui::RichText::new(format!(
                                    "▲ {} notes above view (Click to expand view)",
                                    notes_above
                                ))
                                .color(Color32::from_rgb(255, 200, 60)),
                            )
                            .clicked()
                        {
                            state.num_notes_displayed = (state.num_notes_displayed + 24).min(84);
                        }
                    }
                    if notes_below > 0 {
                        if ui
                            .button(
                                egui::RichText::new(format!(
                                    "▼ {} notes below view (Click to expand view)",
                                    notes_below
                                ))
                                .color(Color32::from_rgb(255, 200, 60)),
                            )
                            .clicked()
                        {
                            state.base_midi_note = state.base_midi_note.saturating_sub(12);
                            state.num_notes_displayed = (state.num_notes_displayed + 12).min(84);
                        }
                    }
                });
                ui.add_space(2.0);
            }
        }

        // Find which MIDI notes are currently actively hitting at `current_playing_step`
        let mut hitting_notes: Vec<(usize, u8)> = Vec::new(); // (track_idx, note)
        for (t_idx, t) in tracks.iter().enumerate() {
            if !t.muted {
                for n in &t.notes {
                    if current_playing_step >= n.step
                        && current_playing_step < n.step + n.duration_steps
                    {
                        hitting_notes.push((t_idx, n.note));
                    }
                }
            }
        }

        // Scroll Area containing Full Multi-Octave Timeline Canvas
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .max_height(ui.available_height() - 10.0)
            .show(ui, |ui| {
                let total_width = 85.0 + (total_steps as f32 * state.step_width);
                let total_height = 26.0 + (num_notes_displayed as f32 * state.key_height);

                let (response, painter) = ui.allocate_painter(
                    Vec2::new(total_width, total_height),
                    egui::Sense::click_and_drag(),
                );
                let origin = response.rect.min;

                // 1. Draw Background Timeline Header (Steps and Beats)
                let header_rect = Rect::from_min_size(origin, Vec2::new(total_width, 26.0));
                painter.rect_filled(header_rect, 0.0, Color32::from_rgb(18, 22, 32));

                for step in 0..total_steps {
                    let x = origin.x + 85.0 + (step as f32 * state.step_width);
                    let is_beat = (step % steps_per_beat) == 0;

                    let line_color = if is_beat {
                        Color32::from_rgb(70, 88, 125)
                    } else {
                        Color32::from_rgb(32, 40, 58)
                    };

                    painter.line_segment(
                        [
                            Pos2::new(x, origin.y),
                            Pos2::new(x, origin.y + total_height),
                        ],
                        Stroke::new(if is_beat { 1.5_f32 } else { 1.0_f32 }, line_color),
                    );

                    if is_beat {
                        painter.text(
                            Pos2::new(x + 4.0, origin.y + 13.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{}", (step / steps_per_beat) + 1),
                            egui::FontId::monospace(12.0),
                            Color32::from_rgb(200, 220, 255),
                        );
                    }
                }

                // 2. Draw Active Step Column Highlight Beam (Playhead background column)
                if current_playing_step < total_steps {
                    let beam_x = origin.x + 85.0 + (current_playing_step as f32 * state.step_width);
                    let beam_rect = Rect::from_min_size(
                        Pos2::new(beam_x, origin.y),
                        Vec2::new(state.step_width, total_height),
                    );
                    painter.rect_filled(
                        beam_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(255, 220, 80, 28),
                    );
                }

                // 3. Draw Piano Keys on Left and Grid Rows
                let note_names = [
                    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                ];
                for row in 0..num_notes_displayed {
                    let midi_note = base_midi_note + (num_notes_displayed - 1 - row) as u8;
                    let y = origin.y + 26.0 + (row as f32 * state.key_height);
                    let note_idx = (midi_note % 12) as usize;
                    let is_black = [1, 3, 6, 8, 10].contains(&note_idx);

                    // Row background with high contrast
                    let row_rect = Rect::from_min_size(
                        Pos2::new(origin.x + 85.0, y),
                        Vec2::new(total_width - 85.0, state.key_height),
                    );
                    let row_bg = if is_black {
                        Color32::from_rgb(14, 17, 25)
                    } else {
                        Color32::from_rgb(22, 27, 40)
                    };
                    painter.rect_filled(row_rect, 0.0, row_bg);

                    // Octave boundary line (C note top border)
                    let is_c_boundary = note_idx == 0;
                    painter.line_segment(
                        [
                            Pos2::new(origin.x, y + state.key_height),
                            Pos2::new(origin.x + total_width, y + state.key_height),
                        ],
                        Stroke::new(
                            if is_c_boundary { 1.8_f32 } else { 1.0_f32 },
                            if is_c_boundary {
                                Color32::from_rgb(60, 80, 120)
                            } else {
                                Color32::from_rgb(34, 42, 60)
                            },
                        ),
                    );

                    // Check if this key is currently hitting right now!
                    let is_hitting = hitting_notes.iter().any(|(_, n)| *n == midi_note);

                    // Piano Key button on left
                    let key_rect = Rect::from_min_size(
                        Pos2::new(origin.x, y),
                        Vec2::new(85.0, state.key_height),
                    );
                    let key_color = if is_hitting {
                        Color32::from_rgb(255, 220, 60) // Bright Gold Active Glow
                    } else if is_black {
                        Color32::from_rgb(32, 38, 50)
                    } else {
                        Color32::from_rgb(220, 230, 245)
                    };

                    let key_text_color = if is_hitting {
                        Color32::BLACK
                    } else if is_black {
                        Color32::WHITE
                    } else {
                        Color32::BLACK
                    };

                    painter.rect_filled(key_rect, 2.0, key_color);
                    let label = format!("{}{}", note_names[note_idx], (midi_note / 12) as i32 - 1);
                    painter.text(
                        Pos2::new(origin.x + 8.0, y + state.key_height * 0.5),
                        egui::Align2::LEFT_CENTER,
                        label,
                        egui::FontId::proportional(11.0),
                        key_text_color,
                    );
                }

                // 4. Render Ghost Notes (Other Tracks) for Full Arrangement View
                if state.show_ghost_notes {
                    for (t_idx, other_track) in tracks.iter().enumerate() {
                        if t_idx == state.selected_track_idx || other_track.muted {
                            continue;
                        }
                        let ghost_color =
                            TRACK_COLORS[t_idx % TRACK_COLORS.len()].gamma_multiply(0.35);

                        for note in &other_track.notes {
                            if note.note >= base_midi_note
                                && (note.note as usize)
                                    < (base_midi_note as usize + num_notes_displayed)
                            {
                                let row = (num_notes_displayed - 1)
                                    - (note.note - base_midi_note) as usize;
                                let nx = origin.x + 85.0 + (note.step as f32 * state.step_width);
                                let ny = origin.y + 26.0 + (row as f32 * state.key_height);
                                let nw =
                                    (note.duration_steps as f32 * state.step_width).max(12.0) - 2.0;
                                let nh = state.key_height - 2.0;

                                let note_rect = Rect::from_min_size(
                                    Pos2::new(nx + 1.0, ny + 1.0),
                                    Vec2::new(nw, nh),
                                );
                                painter.rect_filled(note_rect, 2.0, ghost_color);
                            }
                        }
                    }
                }

                // 5. Render Notes for Selected Active Track
                if let Some(track) = tracks.get_mut(state.selected_track_idx) {
                    let mut note_to_remove = None;

                    for (idx, note) in track.notes.iter().enumerate() {
                        if note.note >= base_midi_note
                            && (note.note as usize)
                                < (base_midi_note as usize + num_notes_displayed)
                        {
                            let row =
                                (num_notes_displayed - 1) - (note.note - base_midi_note) as usize;
                            let nx = origin.x + 85.0 + (note.step as f32 * state.step_width);
                            let ny = origin.y + 26.0 + (row as f32 * state.key_height);
                            let nw =
                                (note.duration_steps as f32 * state.step_width).max(12.0) - 2.0;
                            let nh = state.key_height - 2.0;

                            let note_rect = Rect::from_min_size(
                                Pos2::new(nx + 1.0, ny + 1.0),
                                Vec2::new(nw, nh),
                            );

                            // Check if this specific note is currently being played by playhead
                            let is_active_hit = current_playing_step >= note.step
                                && current_playing_step < note.step + note.duration_steps;

                            let (fill_col, stroke_col, stroke_w) = if is_active_hit {
                                (
                                    Color32::from_rgb(255, 235, 90),  // Blazing Gold on Note Hit!
                                    Color32::from_rgb(255, 255, 255), // White Glow Border
                                    2.5_f32,
                                )
                            } else {
                                (
                                    active_track_color,
                                    Color32::from_rgb(220, 240, 255),
                                    1.0_f32,
                                )
                            };

                            painter.rect_filled(note_rect, 3.0, fill_col);
                            painter.rect_stroke(
                                note_rect,
                                3.0,
                                Stroke::new(stroke_w, stroke_col),
                                StrokeKind::Inside,
                            );

                            // Note pitch name label inside the note box
                            let note_name = note_names[(note.note % 12) as usize];
                            painter.text(
                                Pos2::new(nx + 4.0, ny + state.key_height * 0.5),
                                egui::Align2::LEFT_CENTER,
                                format!("{}{}", note_name, (note.note / 12) as i32 - 1),
                                egui::FontId::monospace(10.0),
                                Color32::BLACK,
                            );

                            // Right click on note to delete
                            if response.clicked_by(egui::PointerButton::Secondary) {
                                if let Some(mouse_pos) = response.hover_pos() {
                                    if note_rect.contains(mouse_pos) {
                                        note_to_remove = Some(idx);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(rem_idx) = note_to_remove {
                        track.notes.remove(rem_idx);
                    }

                    // Left Click to place or remove a note
                    if response.clicked_by(egui::PointerButton::Primary) {
                        if let Some(mouse_pos) = response.hover_pos() {
                            if mouse_pos.x > origin.x + 85.0 && mouse_pos.y > origin.y + 26.0 {
                                let step =
                                    ((mouse_pos.x - (origin.x + 85.0)) / state.step_width) as u32;
                                let row =
                                    ((mouse_pos.y - (origin.y + 26.0)) / state.key_height) as usize;
                                if step < total_steps && row < num_notes_displayed {
                                    let midi_note =
                                        base_midi_note + (num_notes_displayed - 1 - row) as u8;

                                    // Remove existing note at same step & pitch if any, otherwise add
                                    if let Some(pos) = track
                                        .notes
                                        .iter()
                                        .position(|n| n.step == step && n.note == midi_note)
                                    {
                                        track.notes.remove(pos);
                                    } else {
                                        track.notes.push(NoteEvent {
                                            step,
                                            note: midi_note,
                                            duration_steps: 2,
                                            velocity: state.current_velocity,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                // 6. Draw High-Visibility Playhead Line & Top Marker
                if current_playing_step < total_steps {
                    let playhead_x =
                        origin.x + 85.0 + (current_playing_step as f32 * state.step_width);

                    // Solid bright red needle
                    painter.line_segment(
                        [
                            Pos2::new(playhead_x, origin.y),
                            Pos2::new(playhead_x, origin.y + total_height),
                        ],
                        Stroke::new(3.0_f32, Color32::from_rgb(255, 60, 60)),
                    );

                    // Header marker needle
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(playhead_x - 3.0, origin.y),
                            Vec2::new(6.0, 10.0),
                        ),
                        2.0,
                        Color32::from_rgb(255, 90, 90),
                    );
                }
            });
    });
}
