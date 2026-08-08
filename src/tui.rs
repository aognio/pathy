use std::collections::HashSet;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::path::{EntryType, PathEntry, format_modified, format_size, inspect_path};

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn run(entries: Vec<PathEntry>) -> io::Result<()> {
    let mut terminal = TerminalGuard::enter()?;
    let mut app = TuiApp::new(entries);

    loop {
        terminal.draw(|frame| render(frame, &mut app))?;

        if event::poll(Duration::from_millis(250))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };

            if app.handle_key(key) {
                break;
            }
        }
    }

    let export = app.export_after_quit.then(|| app.export_path());
    drop(terminal);

    if let Some(export) = export {
        println!("{export}");
    }

    Ok(())
}

struct TerminalGuard {
    terminal: TuiTerminal,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Fixed(tui_viewport()),
            },
        )?;

        Ok(Self { terminal })
    }
}

fn tui_viewport() -> Rect {
    let width = std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(120);
    let height = std::env::var("LINES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(40);

    Rect::new(0, 0, width.max(40), height.max(12))
}

impl std::ops::Deref for TerminalGuard {
    type Target = TuiTerminal;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl std::ops::DerefMut for TerminalGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[derive(Debug, Eq, PartialEq)]
enum InputMode {
    Normal,
    Insert { buffer: String },
    Goto { buffer: String },
}

#[derive(Debug)]
struct TuiApp {
    entries: Vec<PathEntry>,
    selected: usize,
    table_state: TableState,
    input_mode: InputMode,
    message: String,
    export_after_quit: bool,
}

impl TuiApp {
    fn new(entries: Vec<PathEntry>) -> Self {
        let mut app = Self {
            entries,
            selected: 0,
            table_state: TableState::default(),
            input_mode: InputMode::Normal,
            message: "Use arrows or j/k to move. Press ? for keys.".to_string(),
            export_after_quit: false,
        };
        app.normalize();
        app
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match &mut self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Insert { buffer } => match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.message = "Insert cancelled.".to_string();
                    false
                }
                KeyCode::Enter => {
                    let value = std::mem::take(buffer);
                    self.input_mode = InputMode::Normal;
                    self.insert_path(value);
                    false
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    false
                }
                KeyCode::Char(value) => {
                    buffer.push(value);
                    false
                }
                _ => false,
            },
            InputMode::Goto { buffer } => match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.message = "Jump cancelled.".to_string();
                    false
                }
                KeyCode::Enter => {
                    let value = std::mem::take(buffer);
                    self.input_mode = InputMode::Normal;
                    self.goto_row(value);
                    false
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    false
                }
                KeyCode::Char(value) if value.is_ascii_digit() => {
                    buffer.push(value);
                    false
                }
                _ => false,
            },
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q') | KeyCode::Esc, _) => true,
            (KeyCode::Char('x'), _) => {
                self.export_after_quit = true;
                true
            }
            (KeyCode::Up, KeyModifiers::CONTROL) | (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                self.move_selected_up();
                false
            }
            (KeyCode::Down, KeyModifiers::CONTROL)
            | (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                self.move_selected_down();
                false
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                self.select_previous();
                false
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                self.select_next();
                false
            }
            (KeyCode::Home, _) => {
                self.select_first();
                false
            }
            (KeyCode::End, _) => {
                self.select_last();
                false
            }
            (KeyCode::Char('d'), _) | (KeyCode::Delete, _) => {
                self.remove_selected();
                false
            }
            (KeyCode::Char('i'), _) => {
                self.input_mode = InputMode::Insert {
                    buffer: String::new(),
                };
                self.message = "Insert directory path, then Enter.".to_string();
                false
            }
            (KeyCode::Char('g'), _) => {
                self.input_mode = InputMode::Goto {
                    buffer: String::new(),
                };
                self.message = "Jump to row number, then Enter.".to_string();
                false
            }
            (KeyCode::Char('D'), _) => {
                self.remove_duplicates();
                false
            }
            (KeyCode::Char('M'), _) => {
                self.remove_missing();
                false
            }
            (KeyCode::Char('N'), _) => {
                self.remove_non_directories();
                false
            }
            (KeyCode::Char('C'), _) => {
                self.cleanup();
                false
            }
            (KeyCode::Char('?'), _) => {
                self.message = "Keys: q quit, x export, j/k move, Ctrl+j/k reorder, i insert, g jump, d delete, D dedupe, M missing, N non-dir, C clean.".to_string();
                false
            }
            _ => false,
        }
    }

    fn select_previous(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        self.selected = self.selected.saturating_sub(1);
        self.sync_table_state();
    }

    fn select_next(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        self.selected = (self.selected + 1).min(self.entries.len() - 1);
        self.sync_table_state();
    }

    fn select_first(&mut self) {
        self.selected = 0;
        self.sync_table_state();
    }

    fn select_last(&mut self) {
        self.selected = self.entries.len().saturating_sub(1);
        self.sync_table_state();
    }

    fn move_selected_up(&mut self) {
        if self.selected == 0 || self.entries.len() < 2 {
            return;
        }

        self.entries.swap(self.selected, self.selected - 1);
        self.selected -= 1;
        self.normalize();
        self.message = "Moved row up.".to_string();
    }

    fn move_selected_down(&mut self) {
        if self.selected + 1 >= self.entries.len() {
            return;
        }

        self.entries.swap(self.selected, self.selected + 1);
        self.selected += 1;
        self.normalize();
        self.message = "Moved row down.".to_string();
    }

    fn remove_selected(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        let removed = self.entries.remove(self.selected);
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.normalize();
        self.message = format!("Removed {}.", removed.path.display());
    }

    fn insert_path(&mut self, value: String) {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            self.message = "No path inserted.".to_string();
            return;
        }

        let path = PathBuf::from(trimmed);

        if !path.is_dir() {
            self.message = format!(
                "Insert rejected: {} is not an existing directory.",
                path.display()
            );
            return;
        }

        let insert_at = if self.entries.is_empty() {
            0
        } else {
            self.selected + 1
        };

        self.entries.insert(insert_at, entry_for_path(path));
        self.selected = insert_at;
        self.normalize();
        self.message = "Inserted directory.".to_string();
    }

    fn goto_row(&mut self, value: String) {
        let Ok(row_number) = value.parse::<usize>() else {
            self.message = "Invalid row number.".to_string();
            return;
        };

        if row_number == 0 || row_number > self.entries.len() {
            self.message = format!("Row must be between 1 and {}.", self.entries.len());
            return;
        }

        self.selected = row_number - 1;
        self.sync_table_state();
        self.message = format!("Selected row {row_number}.");
    }

    fn remove_duplicates(&mut self) {
        let before = self.entries.len();
        let mut seen = HashSet::new();
        self.entries.retain(|entry| seen.insert(entry.path.clone()));
        self.after_bulk_remove(before, "duplicate");
    }

    fn remove_missing(&mut self) {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.exists);
        self.after_bulk_remove(before, "missing");
    }

    fn remove_non_directories(&mut self) {
        let before = self.entries.len();
        self.entries
            .retain(|entry| matches!(entry.entry_type, EntryType::Directory));
        self.after_bulk_remove(before, "non-directory");
    }

    fn cleanup(&mut self) {
        let before = self.entries.len();
        let mut seen = HashSet::new();
        self.entries.retain(|entry| {
            entry.exists
                && matches!(entry.entry_type, EntryType::Directory)
                && seen.insert(entry.path.clone())
        });
        self.after_bulk_remove(before, "problem");
    }

    fn after_bulk_remove(&mut self, before: usize, label: &str) {
        let removed = before - self.entries.len();
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.normalize();
        self.message = format!("Removed {removed} {label} row(s).");
    }

    fn normalize(&mut self) {
        let mut seen = HashSet::new();

        for (index, entry) in self.entries.iter_mut().enumerate() {
            entry.index = index + 1;
            entry.duplicate = !seen.insert(entry.path.clone());
        }

        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.sync_table_state();
    }

    fn sync_table_state(&mut self) {
        if self.entries.is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state.select(Some(self.selected));
        }
    }

    fn export_path(&self) -> String {
        let joined = std::env::join_paths(self.entries.iter().map(|entry| entry.path.as_path()))
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();

        format!("export PATH=\"{joined}\"")
    }
}

fn entry_for_path(path: PathBuf) -> PathEntry {
    inspect_path(0, path, false)
}

fn render(frame: &mut ratatui::Frame<'_>, app: &mut TuiApp) {
    let area = centered_area(frame.area(), 94, 86);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    frame.render_widget(Clear, area);
    render_header(frame, chunks[0], app);
    render_table(frame, chunks[1], app);
    render_footer(frame, chunks[2], app);

    if matches!(
        app.input_mode,
        InputMode::Insert { .. } | InputMode::Goto { .. }
    ) {
        render_prompt(frame, centered_area(frame.area(), 68, 20), app);
    }
}

fn centered_area(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

fn render_header(frame: &mut ratatui::Frame<'_>, area: Rect, app: &TuiApp) {
    let existing = app.entries.iter().filter(|entry| entry.exists).count();
    let duplicates = app.entries.iter().filter(|entry| entry.duplicate).count();
    let title = Line::from(vec![
        Span::styled(
            "Pathy TUI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "  {} entries | {} existing | {} duplicates",
            app.entries.len(),
            existing,
            duplicates
        )),
    ]);

    frame.render_widget(
        Paragraph::new(title)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center),
        area,
    );
}

fn render_table(frame: &mut ratatui::Frame<'_>, area: Rect, app: &mut TuiApp) {
    let header = Row::new([
        "#", "OK", "PATH", "TYPE", "BIN", "!BIN", "SIZE", "MODIFIED", "DUP",
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows = app.entries.iter().map(|entry| {
        let problem = !entry.exists
            || entry.duplicate
            || !matches!(entry.entry_type, EntryType::Directory)
            || entry.executable_count == 0;
        let style = if problem {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };

        Row::new(vec![
            Cell::from(entry.index.to_string()),
            Cell::from(if entry.exists { "Y" } else { "N" }),
            Cell::from(entry.path.display().to_string()),
            Cell::from(entry.entry_type.label()),
            Cell::from(entry.executable_count.to_string()),
            Cell::from(entry.non_executable_count.to_string()),
            Cell::from(format_size(entry.size_bytes)),
            Cell::from(format_modified(entry.modified)),
            Cell::from(if entry.duplicate { "YES" } else { "" }),
        ])
        .style(style)
    });

    let widths = [
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Min(24),
        Constraint::Length(9),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Length(11),
        Constraint::Length(14),
        Constraint::Length(7),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title(" PATH ").borders(Borders::ALL))
        .row_highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("➤ ");

    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_footer(frame: &mut ratatui::Frame<'_>, area: Rect, app: &TuiApp) {
    let mode = match app.input_mode {
        InputMode::Normal => "NORMAL",
        InputMode::Insert { .. } => "INSERT",
        InputMode::Goto { .. } => "GOTO",
    };
    let text = vec![
        Line::from(vec![
            Span::styled(
                mode,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::raw(app.message.as_str()),
        ]),
        Line::from(
            "q quit | x export+quit | j/k select | Ctrl+j/k move | i insert | g jump | d delete | D/M/N/C cleanup",
        ),
    ];

    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_prompt(frame: &mut ratatui::Frame<'_>, area: Rect, app: &TuiApp) {
    let (title, value) = match &app.input_mode {
        InputMode::Insert { buffer } => ("Insert directory", buffer.as_str()),
        InputMode::Goto { buffer } => ("Jump to row", buffer.as_str()),
        InputMode::Normal => return,
    };

    let prompt = Paragraph::new(value)
        .block(Block::default().title(title).borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, area);
    frame.render_widget(prompt, area);
}

#[cfg(test)]
mod tests {
    use super::{TuiApp, entry_for_path};
    use crate::path::{EntryType, PathEntry};
    use std::path::PathBuf;

    fn entry(index: usize, path: &str, entry_type: EntryType) -> PathEntry {
        PathEntry {
            index,
            path: PathBuf::from(path),
            entry_type,
            exists: !matches!(entry_type, EntryType::Missing),
            executable_count: usize::from(matches!(entry_type, EntryType::Directory)),
            non_executable_count: 0,
            size_bytes: 0,
            duplicate: false,
            modified: None,
        }
    }

    #[test]
    fn move_selected_reorders_and_renumbers() {
        let mut app = TuiApp::new(vec![
            entry(1, "/a", EntryType::Directory),
            entry(2, "/b", EntryType::Directory),
        ]);
        app.selected = 1;

        app.move_selected_up();

        assert_eq!(app.entries[0].path, PathBuf::from("/b"));
        assert_eq!(app.entries[0].index, 1);
        assert_eq!(app.entries[1].index, 2);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn remove_duplicates_keeps_first_occurrence() {
        let mut app = TuiApp::new(vec![
            entry(1, "/a", EntryType::Directory),
            entry(2, "/b", EntryType::Directory),
            entry(3, "/a", EntryType::Directory),
        ]);

        app.remove_duplicates();

        assert_eq!(app.entries.len(), 2);
        assert_eq!(app.entries[0].path, PathBuf::from("/a"));
        assert_eq!(app.entries[1].path, PathBuf::from("/b"));
    }

    #[test]
    fn cleanup_removes_missing_non_directories_and_duplicates() {
        let mut app = TuiApp::new(vec![
            entry(1, "/a", EntryType::Directory),
            entry(2, "/missing", EntryType::Missing),
            entry(3, "/file", EntryType::File),
            entry(4, "/a", EntryType::Directory),
        ]);

        app.cleanup();

        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].path, PathBuf::from("/a"));
    }

    #[test]
    fn goto_row_selects_one_based_row_number() {
        let mut app = TuiApp::new(vec![
            entry(1, "/a", EntryType::Directory),
            entry(2, "/b", EntryType::Directory),
        ]);

        app.goto_row("2".to_string());

        assert_eq!(app.selected, 1);
    }

    #[test]
    fn entry_for_path_accepts_existing_directories() {
        let entry = entry_for_path(std::env::current_dir().expect("current dir"));

        assert!(entry.exists);
        assert_eq!(entry.entry_type, EntryType::Directory);
    }
}
