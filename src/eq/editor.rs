use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;

use crate::devices::Device;
use crate::eq::presets::{
    all_presets, delete_preset, is_builtin, load_preset, load_selected_profile, load_user_presets,
    save_preset, EqPreset,
};
use crate::eq::{DB_MAX, DB_MIN, EQ_FREQUENCIES, NUM_BANDS};

#[derive(PartialEq)]
enum EditorMode {
    StartupConflict,
    Normal,
    PresetSelect,
    PresetSave,
    PresetDelete,
    ConfirmQuit,
}

#[derive(PartialEq, Clone, Copy)]
enum StartupConflictOption {
    OverwriteTui,
    LoadTui,
}

#[derive(PartialEq, Clone, Copy)]
enum ConfirmQuitOption {
    SaveTui,
    SaveAs,
    Undo,
}

/// Result from the EQ editor.
pub enum EditorResult {
    /// User saved: (preset_name, bands). Save preset file + set selected profile.
    Saved { name: String, bands: [f32; NUM_BANDS] },
    /// User cancelled/undid: (preset_name, bands). Restore selected profile + headset state.
    Cancelled { name: String, bands: [f32; NUM_BANDS] },
}

pub struct EqEditor {
    bands: [f32; NUM_BANDS],
    original_bands: [f32; NUM_BANDS],
    original_preset: String,
    reference_bands: [f32; NUM_BANDS],
    cursor: usize,
    modified: bool,
    presets: Vec<EqPreset>,
    active_preset: Option<String>,
    mode: EditorMode,
    preset_list_state: ListState,
    save_input: String,
    confirm_quit_selection: ConfirmQuitOption,
    exit_after_preset_save: bool,
    conflict_preset_name: Option<String>,
    conflict_preset_bands: Option<[f32; NUM_BANDS]>,
    startup_conflict_selection: StartupConflictOption,
}

impl EqEditor {
    pub fn new() -> Self {
        let presets = all_presets();
        let profile = load_selected_profile();
        let selected_name = profile.active_preset;

        // Load TUI state and detect conflicts with selected profile
        let tui_preset = load_preset("TUI");
        let tui_exists = tui_preset.is_some();
        let tui_bands = tui_preset.map(|p| p.bands).unwrap_or([0.0; NUM_BANDS]);

        let (mode, bands, active_name, original_name, conflict_name, conflict_bands) =
            match &selected_name {
                Some(name) if name != "TUI" => {
                    if let Some(selected_preset) = load_preset(name) {
                        if !tui_exists {
                            // No TUI.json yet — just load the selected preset directly
                            (
                                EditorMode::Normal,
                                selected_preset.bands,
                                Some(name.clone()),
                                name.clone(),
                                None,
                                None,
                            )
                        } else if selected_preset.bands != tui_bands {
                            // TUI exists and differs from selected — conflict
                            (
                                EditorMode::StartupConflict,
                                tui_bands,
                                Some("TUI".to_string()),
                                "TUI".to_string(),
                                Some(name.clone()),
                                Some(selected_preset.bands),
                            )
                        } else {
                            // Same bands, no conflict — load selected preset name
                            (
                                EditorMode::Normal,
                                tui_bands,
                                Some(name.clone()),
                                name.clone(),
                                None,
                                None,
                            )
                        }
                    } else {
                        // Selected preset not found — fall back to TUI
                        (
                            EditorMode::Normal,
                            tui_bands,
                            Some("TUI".to_string()),
                            "TUI".to_string(),
                            None,
                            None,
                        )
                    }
                }
                _ => {
                    // Selected is TUI or nothing
                    (
                        EditorMode::Normal,
                        tui_bands,
                        selected_name.clone(),
                        selected_name.unwrap_or_else(|| "TUI".to_string()),
                        None,
                        None,
                    )
                }
            };

        EqEditor {
            bands,
            original_bands: bands,
            original_preset: original_name,
            reference_bands: bands,
            cursor: 0,
            modified: false,
            presets,
            active_preset: active_name,
            mode,
            preset_list_state: ListState::default(),
            save_input: String::new(),
            confirm_quit_selection: ConfirmQuitOption::SaveTui,
            exit_after_preset_save: false,
            conflict_preset_name: conflict_name.clone(),
            conflict_preset_bands: conflict_bands,
            startup_conflict_selection: StartupConflictOption::OverwriteTui,
        }
    }

    pub fn run(mut self, mut device: Option<&mut dyn Device>) -> io::Result<EditorResult> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Install panic hook for terminal restore
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            original_hook(info);
        }));

        let result = self.event_loop(&mut terminal, &mut device);

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

        result
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        device: &mut Option<&mut dyn Device>,
    ) -> io::Result<EditorResult> {
        // Sync EQ to headset on startup (unless conflict dialog is showing)
        if self.mode != EditorMode::StartupConflict {
            self.send_all_bands_to_device(device);
        }

        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if let Event::Key(key) = event::read()? {
                // Ctrl+C: save current state as TUI and exit
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    return Ok(EditorResult::Saved {
                        name: "TUI".to_string(),
                        bands: self.bands,
                    });
                }

                match self.mode {
                    EditorMode::StartupConflict => {
                        self.handle_startup_conflict_key(key, device);
                    }
                    EditorMode::Normal => {
                        if let Some(result) = self.handle_normal_key(key, device) {
                            return Ok(result);
                        }
                    }
                    EditorMode::PresetSelect => self.handle_preset_select_key(key, device),
                    EditorMode::PresetSave => {
                        if let Some(result) = self.handle_preset_save_key(key) {
                            return Ok(result);
                        }
                    }
                    EditorMode::PresetDelete => self.handle_preset_delete_key(key),
                    EditorMode::ConfirmQuit => {
                        if let Some(result) = self.handle_confirm_quit_key(key, device) {
                            return Ok(result);
                        }
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let block = Block::default()
            .title(" HyperX Equalizer Editor ")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(inner);

        // Graph area
        let graph_lines = self.build_graph_lines();
        let graph = Paragraph::new(graph_lines);
        frame.render_widget(graph, chunks[0]);

        // Footer
        let footer = self.build_footer();
        frame.render_widget(footer, chunks[1]);

        // Overlay popups
        match self.mode {
            EditorMode::StartupConflict => self.draw_startup_conflict(frame, area),
            EditorMode::PresetSelect => self.draw_preset_select(frame, area),
            EditorMode::PresetSave => self.draw_preset_save(frame, area),
            EditorMode::PresetDelete => self.draw_preset_delete(frame, area),
            EditorMode::ConfirmQuit => self.draw_confirm_quit(frame, area),
            EditorMode::Normal => {}
        }
    }

    fn build_graph_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        // Band labels
        let mut label_spans = vec![Span::raw("     ")];
        for (i, &(_, label, _)) in EQ_FREQUENCIES.iter().enumerate() {
            let style = if i == self.cursor {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let text = if i == self.cursor {
                format!("[{:^5}]", label)
            } else {
                format!(" {:^5} ", label)
            };
            label_spans.push(Span::styled(text, style));
        }
        lines.push(Line::from(label_spans));
        lines.push(Line::default());

        // Graph rows: +12 to -12 step 2
        for db in (0..=12).rev().chain((-12..0).rev()) {
            if db > 12 || db < -12 {
                continue;
            }
            // Only draw even values
            if db % 2 != 0 {
                continue;
            }

            let mut spans = vec![Span::raw(format!("{:+3} ", db))];

            for i in 0..NUM_BANDS {
                let val = self.bands[i];
                let is_selected = i == self.cursor;
                let db_f = db as f32;

                let (text, color) = if db == 0 {
                    if is_selected {
                        ("\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}", Color::Yellow)
                    } else {
                        ("\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}", Color::Yellow)
                    }
                } else if db > 0 && val >= db_f {
                    ("   \u{2588}   ", Color::Green)
                } else if db < 0 && val <= db_f {
                    ("   \u{2588}   ", Color::Red)
                } else if db > 0 && (db_f - 2.0) < val && val < db_f {
                    ("   \u{2584}   ", Color::Green)
                } else if db < 0 && db_f < val && val < (db_f + 2.0) {
                    ("   \u{2580}   ", Color::Red)
                } else {
                    ("   \u{00b7}   ", Color::DarkGray)
                };

                let mut style = Style::default().fg(color);
                if is_selected {
                    style = style.add_modifier(Modifier::BOLD);
                }
                spans.push(Span::styled(text.to_string(), style));
            }

            lines.push(Line::from(spans));
        }

        lines.push(Line::default());

        // Values row
        let mut val_spans = vec![Span::raw("     ")];
        for i in 0..NUM_BANDS {
            let val = self.bands[i];
            let style = if i == self.cursor {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if val > 0.0 {
                Style::default().fg(Color::Green)
            } else if val < 0.0 {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };
            val_spans.push(Span::styled(format!("{:+5.1}  ", val), style));
        }
        lines.push(Line::from(val_spans));
        lines.push(Line::default());

        // Selected band info
        let (_, freq, desc) = EQ_FREQUENCIES[self.cursor];
        let info = format!(
            "  Selected: {} ({}) = {:+.1} dB",
            freq, desc, self.bands[self.cursor]
        );
        let preset_info = match &self.active_preset {
            Some(name) => format!("  Preset: {}{}", name, if self.modified { " *" } else { "" }),
            None => {
                if self.modified {
                    "  Custom *".to_string()
                } else {
                    String::new()
                }
            }
        };
        lines.push(Line::from(vec![
            Span::styled(
                info,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(preset_info, Style::default().fg(Color::DarkGray)),
        ]));

        lines
    }

    fn build_footer(&self) -> Paragraph<'static> {
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "\u{2190}\u{2192}",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(": Band  "),
                Span::styled(
                    "\u{2191}\u{2193}",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(": \u{00b1}1dB  "),
                Span::styled(
                    "PgUp/Dn",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(": \u{00b1}3dB  "),
                Span::styled("0", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": Reset band  "),
                Span::styled("r", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": Revert  "),
                Span::styled("⇧0", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": Flat"),
            ]),
            Line::from(vec![
                Span::styled("p", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": Presets  "),
                Span::styled("s", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": Save preset  "),
                Span::styled("d", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": Delete preset  "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": Save+Exit  "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("/"),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": Exit"),
            ]),
        ];
        Paragraph::new(lines)
    }

    fn handle_startup_conflict_key(
        &mut self,
        key: KeyEvent,
        device: &mut Option<&mut dyn Device>,
    ) {
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                self.startup_conflict_selection =
                    if self.startup_conflict_selection == StartupConflictOption::OverwriteTui {
                        StartupConflictOption::LoadTui
                    } else {
                        StartupConflictOption::OverwriteTui
                    };
            }
            KeyCode::Enter => {
                match self.startup_conflict_selection {
                    StartupConflictOption::OverwriteTui => {
                        if let Some(bands) = self.conflict_preset_bands {
                            self.bands = bands;
                            self.original_bands = bands;
                            self.reference_bands = bands;
                            self.active_preset = self.conflict_preset_name.clone();
                            self.original_preset = self
                                .conflict_preset_name
                                .clone()
                                .unwrap_or_else(|| "TUI".to_string());
                        }
                    }
                    StartupConflictOption::LoadTui => {
                        // Keep TUI bands as loaded
                    }
                }
                self.conflict_preset_name = None;
                self.conflict_preset_bands = None;
                self.mode = EditorMode::Normal;
                // Now sync chosen state to headset
                self.send_all_bands_to_device(device);
            }
            _ => {}
        }
    }

    fn draw_startup_conflict(&self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(65, 30, area);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Profile Conflict ")
            .borders(Borders::ALL);
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let preset_name = self
            .conflict_preset_name
            .as_deref()
            .unwrap_or("Unknown");

        let btn =
            |label: &str, option: StartupConflictOption, sel_color: Color| -> Span<'static> {
                if self.startup_conflict_selection == option {
                    Span::styled(
                        format!(" {} ", label),
                        Style::default()
                            .fg(Color::Black)
                            .bg(sel_color)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(format!(" {} ", label), Style::default().fg(Color::DarkGray))
                }
            };

        let lines = vec![
            Line::default(),
            Line::from(format!(
                "  Selected profile '{}' differs from TUI state.",
                preset_name
            )),
            Line::default(),
            Line::from(vec![
                Span::raw("   "),
                btn(
                    &format!("Overwrite TUI with '{}'", preset_name),
                    StartupConflictOption::OverwriteTui,
                    Color::Yellow,
                ),
                Span::raw("  "),
                btn("Load TUI", StartupConflictOption::LoadTui, Color::Green),
            ]),
            Line::default(),
            Line::from(Span::styled(
                "  \u{2190}\u{2192}/Tab: Switch  Enter: Confirm",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    /// Returns Some(result) to exit, None to continue
    fn handle_normal_key(
        &mut self,
        key: KeyEvent,
        device: &mut Option<&mut dyn Device>,
    ) -> Option<EditorResult> {
        match key.code {
            KeyCode::Left => {
                self.cursor = (self.cursor + NUM_BANDS - 1) % NUM_BANDS;
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1) % NUM_BANDS;
            }
            KeyCode::Up => {
                self.bands[self.cursor] = (self.bands[self.cursor] + 1.0).min(DB_MAX);
                self.modified = true;
                self.send_band_to_device(device, self.cursor);
            }
            KeyCode::Down => {
                self.bands[self.cursor] = (self.bands[self.cursor] - 1.0).max(DB_MIN);
                self.modified = true;
                self.send_band_to_device(device, self.cursor);
            }
            KeyCode::PageUp => {
                self.bands[self.cursor] = (self.bands[self.cursor] + 3.0).min(DB_MAX);
                self.modified = true;
                self.send_band_to_device(device, self.cursor);
            }
            KeyCode::PageDown => {
                self.bands[self.cursor] = (self.bands[self.cursor] - 3.0).max(DB_MIN);
                self.modified = true;
                self.send_band_to_device(device, self.cursor);
            }
            KeyCode::Char('0') => {
                self.bands[self.cursor] = 0.0;
                self.modified = true;
                self.send_band_to_device(device, self.cursor);
            }
            KeyCode::Char('r') => {
                self.bands = self.reference_bands;
                self.modified = self.bands != self.original_bands;
                self.send_all_bands_to_device(device);
            }
            KeyCode::Char(')') => {
                // Shift+0: reset all bands to flat
                self.bands = [0.0; NUM_BANDS];
                self.modified = true;
                self.active_preset = Some("Flat".to_string());
                self.reference_bands = self.bands;
                self.send_all_bands_to_device(device);
            }
            KeyCode::Char('p') => {
                self.mode = EditorMode::PresetSelect;
                self.preset_list_state.select(Some(
                    self.active_preset
                        .as_ref()
                        .and_then(|name| self.presets.iter().position(|p| p.name == *name))
                        .unwrap_or(0),
                ));
            }
            KeyCode::Char('s') => {
                self.mode = EditorMode::PresetSave;
                self.save_input = self
                    .active_preset
                    .as_ref()
                    .filter(|name| !is_builtin(name))
                    .cloned()
                    .unwrap_or_default();
            }
            KeyCode::Char('d') => {
                let user = load_user_presets();
                if !user.is_empty() {
                    self.mode = EditorMode::PresetDelete;
                    self.preset_list_state.select(Some(0));
                }
            }
            KeyCode::Enter => {
                let name = self.active_preset.clone().unwrap_or_else(|| "TUI".to_string());
                return Some(EditorResult::Saved {
                    name,
                    bands: self.bands,
                });
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.modified {
                    self.confirm_quit_selection = ConfirmQuitOption::SaveTui;
                    self.mode = EditorMode::ConfirmQuit;
                } else {
                    return Some(EditorResult::Cancelled {
                        name: self.original_preset.clone(),
                        bands: self.original_bands,
                    });
                }
            }
            _ => {}
        }
        None
    }

    fn handle_preset_select_key(
        &mut self,
        key: KeyEvent,
        device: &mut Option<&mut dyn Device>,
    ) {
        let len = self.presets.len();
        if len == 0 {
            self.mode = EditorMode::Normal;
            return;
        }

        match key.code {
            KeyCode::Up => {
                let i = self.preset_list_state.selected().unwrap_or(0);
                self.preset_list_state
                    .select(Some(if i == 0 { len - 1 } else { i - 1 }));
            }
            KeyCode::Down => {
                let i = self.preset_list_state.selected().unwrap_or(0);
                self.preset_list_state.select(Some((i + 1) % len));
            }
            KeyCode::Enter => {
                if let Some(i) = self.preset_list_state.selected() {
                    if let Some(preset) = self.presets.get(i) {
                        self.bands = preset.bands;
                        self.reference_bands = preset.bands;
                        self.active_preset = Some(preset.name.clone());
                        self.modified = true;
                        self.send_all_bands_to_device(device);
                    }
                }
                self.mode = EditorMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_preset_save_key(&mut self, key: KeyEvent) -> Option<EditorResult> {
        match key.code {
            KeyCode::Char(c) => {
                if self.save_input.len() < 30 {
                    self.save_input.push(c);
                }
            }
            KeyCode::Backspace => {
                self.save_input.pop();
            }
            KeyCode::Enter => {
                if !self.save_input.is_empty() {
                    let preset = EqPreset {
                        name: self.save_input.clone(),
                        bands: self.bands,
                    };
                    save_preset(&preset).ok();
                    self.active_preset = Some(self.save_input.clone());
                    self.presets = all_presets();

                    if self.exit_after_preset_save {
                        return Some(EditorResult::Saved {
                            name: self.save_input.clone(),
                            bands: self.bands,
                        });
                    }
                }
                self.save_input.clear();
                self.mode = EditorMode::Normal;
            }
            KeyCode::Esc => {
                self.save_input.clear();
                self.exit_after_preset_save = false;
                self.mode = EditorMode::Normal;
            }
            _ => {}
        }
        None
    }

    fn handle_preset_delete_key(&mut self, key: KeyEvent) {
        let user_presets = load_user_presets();
        if user_presets.is_empty() {
            self.mode = EditorMode::Normal;
            return;
        }
        let len = user_presets.len();

        match key.code {
            KeyCode::Up => {
                let i = self.preset_list_state.selected().unwrap_or(0);
                self.preset_list_state
                    .select(Some(if i == 0 { len - 1 } else { i - 1 }));
            }
            KeyCode::Down => {
                let i = self.preset_list_state.selected().unwrap_or(0);
                self.preset_list_state.select(Some((i + 1) % len));
            }
            KeyCode::Enter => {
                if let Some(i) = self.preset_list_state.selected() {
                    if i < user_presets.len() {
                        let name = &user_presets[i].name;
                        delete_preset(name).ok();
                        if self.active_preset.as_deref() == Some(name) {
                            self.active_preset = None;
                        }
                        self.presets = all_presets();
                    }
                }
                self.preset_list_state.select(Some(0));
                self.mode = EditorMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_confirm_quit_key(
        &mut self,
        key: KeyEvent,
        device: &mut Option<&mut dyn Device>,
    ) -> Option<EditorResult> {
        match key.code {
            KeyCode::Right | KeyCode::Tab => {
                self.confirm_quit_selection = match self.confirm_quit_selection {
                    ConfirmQuitOption::SaveTui => ConfirmQuitOption::SaveAs,
                    ConfirmQuitOption::SaveAs => ConfirmQuitOption::Undo,
                    ConfirmQuitOption::Undo => ConfirmQuitOption::SaveTui,
                };
            }
            KeyCode::Left | KeyCode::BackTab => {
                self.confirm_quit_selection = match self.confirm_quit_selection {
                    ConfirmQuitOption::SaveTui => ConfirmQuitOption::Undo,
                    ConfirmQuitOption::SaveAs => ConfirmQuitOption::SaveTui,
                    ConfirmQuitOption::Undo => ConfirmQuitOption::SaveAs,
                };
            }
            KeyCode::Enter => match self.confirm_quit_selection {
                ConfirmQuitOption::SaveTui => {
                    return Some(EditorResult::Saved {
                        name: "TUI".to_string(),
                        bands: self.bands,
                    });
                }
                ConfirmQuitOption::SaveAs => {
                    self.exit_after_preset_save = true;
                    self.mode = EditorMode::PresetSave;
                    self.save_input = self
                        .active_preset
                        .as_ref()
                        .filter(|name| !is_builtin(name))
                        .cloned()
                        .unwrap_or_default();
                }
                ConfirmQuitOption::Undo => {
                    self.restore_original(device);
                    return Some(EditorResult::Cancelled {
                        name: self.original_preset.clone(),
                        bands: self.original_bands,
                    });
                }
            },
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
            }
            _ => {}
        }
        None
    }

    fn draw_confirm_quit(&self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(60, 25, area);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Unsaved Changes ")
            .borders(Borders::ALL);
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let btn = |label: &str, option: ConfirmQuitOption, sel_color: Color| -> Span<'static> {
            if self.confirm_quit_selection == option {
                Span::styled(
                    format!(" {} ", label),
                    Style::default()
                        .fg(Color::Black)
                        .bg(sel_color)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(format!(" {} ", label), Style::default().fg(Color::DarkGray))
            }
        };

        let lines = vec![
            Line::default(),
            Line::from("  You have unsaved EQ changes."),
            Line::default(),
            Line::from(vec![
                Span::raw("   "),
                btn("Save TUI", ConfirmQuitOption::SaveTui, Color::Green),
                Span::raw("  "),
                btn("Save As", ConfirmQuitOption::SaveAs, Color::Cyan),
                Span::raw("  "),
                btn("Undo Changes", ConfirmQuitOption::Undo, Color::Red),
            ]),
            Line::default(),
            Line::from(Span::styled(
                "  \u{2190}\u{2192}/Tab: Switch  Enter: Confirm  Esc: Back",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn send_band_to_device(&self, device: &mut Option<&mut dyn Device>, band: usize) {
        if let Some(ref mut dev) = device {
            if let Some(packets) =
                dev.set_equalizer_bands_packets(&[(band as u8, self.bands[band])])
            {
                for packet in packets {
                    dev.prepare_write();
                    let _ = dev.get_device_state().hid_devices[0].write(&packet);
                }
            }
        }
    }

    fn send_all_bands_to_device(&self, device: &mut Option<&mut dyn Device>) {
        self.send_bands_to_device(device, &self.bands);
    }

    fn restore_original(&self, device: &mut Option<&mut dyn Device>) {
        self.send_bands_to_device(device, &self.original_bands);
    }

    fn send_bands_to_device(&self, device: &mut Option<&mut dyn Device>, bands: &[f32; NUM_BANDS]) {
        if let Some(ref mut dev) = device {
            let pairs: Vec<(u8, f32)> = bands
                .iter()
                .enumerate()
                .map(|(i, &db)| (i as u8, db))
                .collect();
            if let Some(packets) = dev.set_equalizer_bands_packets(&pairs) {
                for packet in packets {
                    dev.prepare_write();
                    let _ = dev.get_device_state().hid_devices[0].write(&packet);
                    std::thread::sleep(std::time::Duration::from_millis(3));
                }
            }
        }
    }

    fn draw_preset_select(&mut self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(40, 60, area);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Select Preset ")
            .borders(Borders::ALL);

        let items: Vec<ListItem> = self
            .presets
            .iter()
            .map(|p| {
                let suffix = if is_builtin(&p.name) { "" } else { " (custom)" };
                let style = if Some(&p.name) == self.active_preset.as_ref() {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}{}", p.name, suffix)).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{25b6} ");

        frame.render_stateful_widget(list, popup, &mut self.preset_list_state);
    }

    fn draw_preset_save(&self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(50, 25, area);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Save Preset ")
            .borders(Borders::ALL);
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let lines = vec![
            Line::default(),
            Line::from(vec![
                Span::raw("  Name: "),
                Span::styled(
                    format!("{}_", self.save_input),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::default(),
            Line::from(Span::styled(
                "  Enter: Save  Esc: Cancel",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn draw_preset_delete(&mut self, frame: &mut Frame, area: Rect) {
        let user_presets = load_user_presets();
        let popup = centered_rect(40, 50, area);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Delete Preset ")
            .borders(Borders::ALL);

        let items: Vec<ListItem> = user_presets
            .iter()
            .map(|p| {
                let suffix = if is_builtin(&p.name) {
                    " (restore default)"
                } else {
                    ""
                };
                ListItem::new(format!("{}{}", p.name, suffix))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::Red)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{25b6} ");

        frame.render_stateful_widget(list, popup, &mut self.preset_list_state);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
