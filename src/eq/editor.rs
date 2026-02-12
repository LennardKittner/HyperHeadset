use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
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
    all_presets, is_builtin, load_settings, load_user_presets, save_user_presets,
    EqPreset, EqSettings,
};
use crate::eq::{DB_MAX, DB_MIN, EQ_FREQUENCIES, NUM_BANDS};

#[derive(PartialEq)]
enum EditorMode {
    Normal,
    PresetSelect,
    PresetSave,
    PresetDelete,
}

pub struct EqEditor {
    bands: [f32; NUM_BANDS],
    original_bands: [f32; NUM_BANDS],
    cursor: usize,
    modified: bool,
    presets: Vec<EqPreset>,
    active_preset: Option<String>,
    mode: EditorMode,
    preset_list_state: ListState,
    save_input: String,
}

impl EqEditor {
    pub fn new() -> Self {
        let presets = all_presets();
        let settings = load_settings();
        let active = settings.active_preset.clone();

        EqEditor {
            bands: settings.bands,
            original_bands: settings.bands,
            cursor: 0,
            modified: false,
            presets,
            active_preset: active,
            mode: EditorMode::Normal,
            preset_list_state: ListState::default(),
            save_input: String::new(),
        }
    }

    pub fn run(mut self, mut device: Option<&mut dyn Device>) -> io::Result<Option<EqSettings>> {
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
    ) -> io::Result<Option<EqSettings>> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if let Event::Key(key) = event::read()? {
                match self.mode {
                    EditorMode::Normal => {
                        if let Some(result) = self.handle_normal_key(key, device) {
                            return Ok(result);
                        }
                    }
                    EditorMode::PresetSelect => self.handle_preset_select_key(key, device),
                    EditorMode::PresetSave => self.handle_preset_save_key(key),
                    EditorMode::PresetDelete => self.handle_preset_delete_key(key),
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
            EditorMode::PresetSelect => self.draw_preset_select(frame, area),
            EditorMode::PresetSave => self.draw_preset_save(frame, area),
            EditorMode::PresetDelete => self.draw_preset_delete(frame, area),
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
                Span::raw(": Reset all"),
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
                Span::raw(": Cancel"),
            ]),
        ];
        Paragraph::new(lines)
    }

    /// Returns Some(result) to exit, None to continue
    fn handle_normal_key(
        &mut self,
        key: KeyEvent,
        device: &mut Option<&mut dyn Device>,
    ) -> Option<Option<EqSettings>> {
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
                self.bands = [0.0; NUM_BANDS];
                self.modified = true;
                self.active_preset = Some("Flat".to_string());
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
                let settings = EqSettings {
                    bands: self.bands,
                    active_preset: self.active_preset.clone(),
                };
                return Some(Some(settings));
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.modified {
                    self.restore_original(device);
                }
                return Some(None);
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

    fn handle_preset_save_key(&mut self, key: KeyEvent) {
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
                    let mut user_presets = load_user_presets();
                    if let Some(existing) =
                        user_presets.iter_mut().find(|p| p.name == preset.name)
                    {
                        *existing = preset;
                    } else {
                        user_presets.push(preset);
                    }
                    save_user_presets(&user_presets).ok();
                    self.active_preset = Some(self.save_input.clone());
                    self.presets = all_presets();
                }
                self.save_input.clear();
                self.mode = EditorMode::Normal;
            }
            KeyCode::Esc => {
                self.save_input.clear();
                self.mode = EditorMode::Normal;
            }
            _ => {}
        }
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
                        let name = user_presets[i].name.clone();
                        let new_presets: Vec<_> =
                            user_presets.into_iter().filter(|p| p.name != name).collect();
                        save_user_presets(&new_presets).ok();
                        self.presets = all_presets();
                        if self.active_preset.as_deref() == Some(&name) {
                            self.active_preset = None;
                        }
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

    fn send_band_to_device(&self, device: &mut Option<&mut dyn Device>, band: usize) {
        if let Some(ref mut dev) = device {
            if let Some(packet) =
                dev.set_equalizer_bands_packet(&[(band as u8, self.bands[band])])
            {
                dev.prepare_write();
                let _ = dev.get_device_state().hid_devices[0].write(&packet);
            }
        }
    }

    fn send_all_bands_to_device(&self, device: &mut Option<&mut dyn Device>) {
        if let Some(ref mut dev) = device {
            let pairs: Vec<(u8, f32)> = self
                .bands
                .iter()
                .enumerate()
                .map(|(i, &db)| (i as u8, db))
                .collect();
            if let Some(packet) = dev.set_equalizer_bands_packet(&pairs) {
                dev.prepare_write();
                let _ = dev.get_device_state().hid_devices[0].write(&packet);
            }
        }
    }

    fn restore_original(&self, device: &mut Option<&mut dyn Device>) {
        if let Some(ref mut dev) = device {
            let pairs: Vec<(u8, f32)> = self
                .original_bands
                .iter()
                .enumerate()
                .map(|(i, &db)| (i as u8, db))
                .collect();
            if let Some(packet) = dev.set_equalizer_bands_packet(&pairs) {
                dev.prepare_write();
                let _ = dev.get_device_state().hid_devices[0].write(&packet);
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
