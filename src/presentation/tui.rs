//! Synapsis TUI - Minimal terminal UI

use crate::domain::{
    entities::{Observation, SearchParams, SessionSummary},
    ports::{SessionPort, StoragePort},
};
use crate::SynapsisError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TuiCommand {
    AddObservation(String),
    Search(String),
    ViewSession(String),
    ListSessions,
    ShowStats,
    Quit,
}

pub struct Tui {
    pub storage: Arc<dyn StoragePort>,
    pub sessions: Arc<dyn SessionPort>,
    pub state: TuiState,
}

#[derive(Debug, Clone, Default)]
pub struct TuiState {
    pub mode: AppMode,
    pub observations: Vec<Observation>,
    pub sessions: Vec<SessionSummary>,
    pub search_query: String,
    pub search_results: Vec<Observation>,
    pub selected_index: usize,
    pub input_buffer: String,
    pub message: Option<String>,
    pub stats: Option<TuiStats>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppMode {
    Timeline,
    AddObservation,
    Search,
    Sessions,
    Stats,
    ConfirmQuit,
}

impl Default for AppMode {
    fn default() -> Self {
        Self::Timeline
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiStats {
    pub total_observations: usize,
    pub total_sessions: usize,
    pub storage_size_bytes: u64,
}

impl Tui {
    pub fn new(storage: Arc<dyn StoragePort>, sessions: Arc<dyn SessionPort>) -> Self {
        Self {
            storage,
            sessions,
            state: TuiState::default(),
        }
    }

    #[cfg(not(feature = "tui"))]
    pub fn run(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        Err(Box::new(SynapsisError::internal_unimplemented()))
    }

    fn refresh_data(&mut self) -> crate::domain::errors::Result<()> {
        let entries = self.storage.get_timeline(1000)?;
        self.state.observations = entries.into_iter().map(|e| e.observation).collect();
        self.state.selected_index = 0;
        Ok(())
    }

    fn perform_search(&mut self) -> crate::domain::errors::Result<()> {
        let params = SearchParams::new(&self.state.search_query).with_limit(50);
        let results = self.storage.search_observations(&params)?;
        self.state.search_results = results.into_iter().map(|r| r.observation).collect();
        self.state.selected_index = 0;
        Ok(())
    }

    fn calculate_stats(&mut self) -> crate::domain::errors::Result<()> {
        let entries = self.storage.get_timeline(0)?;
        let sessions = self.sessions.list_sessions()?;

        let obs_size: u64 = entries
            .iter()
            .map(|e| {
                serde_json::to_string(&e.observation)
                    .map(|s| s.len() as u64)
                    .unwrap_or(0)
            })
            .sum();

        self.state.stats = Some(TuiStats {
            total_observations: entries.len(),
            total_sessions: sessions.len(),
            storage_size_bytes: obs_size,
        });

        Ok(())
    }
}

#[cfg(feature = "tui")]
mod tui_impl {
    use super::*;
    use crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{
        backend::CrosstermBackend,
        layout::{Constraint, Direction, Layout, Rect},
        style::{Color, Modifier, Style},
        widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Row, Table},
        Frame, Terminal,
    };
    use std::io;

    impl Tui {
        pub fn run(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
            enable_raw_mode()?;
            let mut stdout = io::stdout();
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
            let backend = CrosstermBackend::new(stdout);
            let mut terminal = Terminal::new(backend)?;

            let res = self.run_internal(&mut terminal);

            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;

            if let Err(e) = res {
                eprintln!("Error: {}", e);
            }
            Ok(())
        }

        fn run_internal(
            &mut self,
            terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        ) -> crate::domain::errors::Result<()> {
            self.refresh_data()?;

            loop {
                terminal.draw(|f| self.render(f))?;

                if let event::Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match self.state.mode {
                            AppMode::Timeline => match key.code {
                                KeyCode::Char('q') => self.state.mode = AppMode::ConfirmQuit,
                                KeyCode::Char('a') => {
                                    self.state.mode = AppMode::AddObservation;
                                    self.state.input_buffer.clear();
                                }
                                KeyCode::Char('s') => {
                                    self.state.mode = AppMode::Search;
                                    self.state.input_buffer.clear();
                                    self.state.search_query.clear();
                                    self.state.search_results.clear();
                                }
                                KeyCode::Char('l') => {
                                    self.state.mode = AppMode::Sessions;
                                    if let Ok(sessions) = self.sessions.list_sessions() {
                                        self.state.sessions = sessions;
                                    }
                                }
                                KeyCode::Char('t') => {
                                    self.refresh_data().ok();
                                }
                                KeyCode::Char('S') => {
                                    self.state.mode = AppMode::Stats;
                                    self.calculate_stats().ok();
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if self.state.selected_index > 0 {
                                        self.state.selected_index -= 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let max = self.state.observations.len().saturating_sub(1);
                                    if self.state.selected_index < max {
                                        self.state.selected_index += 1;
                                    }
                                }
                                _ => {}
                            },
                            AppMode::AddObservation => match key.code {
                                KeyCode::Enter => {
                                    if !self.state.input_buffer.is_empty() {
                                        self.state.message = Some(
                                            "Create session first with 'l' to add observations"
                                                .to_string(),
                                        );
                                        self.state.input_buffer.clear();
                                        self.state.mode = AppMode::Timeline;
                                    }
                                }
                                KeyCode::Char(c) => {
                                    self.state.input_buffer.push(c);
                                }
                                KeyCode::Backspace => {
                                    self.state.input_buffer.pop();
                                }
                                KeyCode::Esc => {
                                    self.state.input_buffer.clear();
                                    self.state.mode = AppMode::Timeline;
                                }
                                _ => {}
                            },
                            AppMode::Search => match key.code {
                                KeyCode::Enter => {
                                    if !self.state.input_buffer.is_empty() {
                                        self.state.search_query = self.state.input_buffer.clone();
                                        self.perform_search().ok();
                                    }
                                }
                                KeyCode::Char(c) => {
                                    self.state.input_buffer.push(c);
                                }
                                KeyCode::Backspace => {
                                    self.state.input_buffer.pop();
                                }
                                KeyCode::Esc => {
                                    self.state.input_buffer.clear();
                                    self.state.search_query.clear();
                                    self.state.search_results.clear();
                                    self.state.mode = AppMode::Timeline;
                                }
                                _ => {}
                            },
                            AppMode::Sessions => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.state.mode = AppMode::Timeline;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if self.state.selected_index > 0 {
                                        self.state.selected_index -= 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let max = self.state.sessions.len().saturating_sub(1);
                                    if self.state.selected_index < max {
                                        self.state.selected_index += 1;
                                    }
                                }
                                KeyCode::Char('r') => {
                                    if let Ok(sessions) = self.sessions.list_sessions() {
                                        self.state.sessions = sessions;
                                    }
                                }
                                _ => {}
                            },
                            AppMode::Stats => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.state.mode = AppMode::Timeline;
                                }
                                KeyCode::Char('r') => {
                                    self.calculate_stats().ok();
                                }
                                _ => {}
                            },
                            AppMode::ConfirmQuit => {
                                if let KeyCode::Char('y') | KeyCode::Enter = key.code {
                                    break;
                                }
                                if let KeyCode::Char('n') | KeyCode::Esc = key.code {
                                    self.state.mode = AppMode::Timeline;
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        }

        fn render(&self, f: &mut Frame) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(f.area());

            self.render_header(f, chunks[0]);

            match self.state.mode {
                AppMode::Timeline => self.render_timeline(f, chunks[1]),
                AppMode::AddObservation => self.render_add_observation(f, chunks[1]),
                AppMode::Search => self.render_search(f, chunks[1]),
                AppMode::Sessions => self.render_sessions(f, chunks[1]),
                AppMode::Stats => self.render_stats(f, chunks[1]),
                AppMode::ConfirmQuit => self.render_confirm_quit(f, chunks[1]),
            }

            self.render_footer(f, chunks[2]);
        }

        fn render_header(&self, f: &mut Frame, area: Rect) {
            let title = match self.state.mode {
                AppMode::Timeline => "Synapsis - Timeline",
                AppMode::AddObservation => "Synapsis - Add Observation",
                AppMode::Search => "Synapsis - Search",
                AppMode::Sessions => "Synapsis - Sessions",
                AppMode::Stats => "Synapsis - Statistics",
                AppMode::ConfirmQuit => "Synapsis - Confirm Quit",
            };

            let help_text = match self.state.mode {
                AppMode::Timeline => {
                    "[a]dd  [s]earch  [l]ist sessions  [S]tats  [t]refresh  [q]uit"
                }
                AppMode::AddObservation => "[Enter] save  [Esc] cancel",
                AppMode::Search => "[Enter] search  [Esc] cancel",
                AppMode::Sessions => "[k/j] navigate  [r] refresh  [q/Esc] back",
                AppMode::Stats => "[r] refresh  [q/Esc] back",
                AppMode::ConfirmQuit => "[y] yes  [n] no",
            };

            let block = Block::default()
                .title(format!(" {} ", title))
                .title_style(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .style(Style::new().bg(Color::DarkGray));

            f.render_widget(block, area);

            let text = Paragraph::new(help_text).style(Style::new().fg(Color::White));
            let inner = Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: 1,
            };
            f.render_widget(text, inner);
        }

        fn render_timeline(&self, f: &mut Frame, area: Rect) {
            if self.state.observations.is_empty() {
                let text = Paragraph::new("No observations yet. Press 'a' to add one.")
                    .style(Style::new().fg(Color::Gray));
                f.render_widget(text, area);
                return;
            }

            let items: Vec<ListItem> = self
                .state
                .observations
                .iter()
                .map(|obs| {
                    let truncated = if obs.content.len() > 60 {
                        format!("{}...", &obs.content[..60])
                    } else {
                        obs.content.clone()
                    };
                    let content = format!(
                        "[{}] {} | {} | {}",
                        obs.created_at.0,
                        obs.session_id.as_str(),
                        obs.observation_type,
                        truncated
                    );
                    ListItem::new(content)
                })
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(self.state.selected_index));

            let list = List::new(items)
                .block(Block::default().title(" Timeline ").borders(Borders::ALL))
                .highlight_style(Style::new().bg(Color::Blue).fg(Color::White))
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, area, &mut list_state);
        }

        fn render_add_observation(&self, f: &mut Frame, area: Rect) {
            let block = Block::default()
                .title(" Enter observation ")
                .borders(Borders::ALL)
                .style(Style::new().bg(Color::Black));

            f.render_widget(block, area);

            let inner = Rect {
                x: area.x + 2,
                y: area.y + 1,
                width: area.width.saturating_sub(4),
                height: 1,
            };

            let input = Paragraph::new(self.state.input_buffer.as_str())
                .style(Style::new().fg(Color::White));

            f.render_widget(input, inner);

            let cursor_pos = ratatui::layout::Position::new(
                inner.x + self.state.input_buffer.len() as u16,
                inner.y,
            );
            f.set_cursor_position(cursor_pos);
        }

        fn render_search(&self, f: &mut Frame, area: Rect) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(area);

            let input_block = Block::default()
                .title(" Search Query ")
                .borders(Borders::ALL);

            f.render_widget(input_block, chunks[0]);

            let input = Paragraph::new(format!("Search: {}", self.state.input_buffer))
                .style(Style::new().fg(Color::White));

            let inner = Rect {
                x: chunks[0].x + 2,
                y: chunks[0].y + 1,
                width: chunks[0].width.saturating_sub(4),
                height: 1,
            };
            f.render_widget(input, inner);

            let cursor_pos = ratatui::layout::Position::new(
                inner.x + 8 + self.state.input_buffer.len() as u16,
                inner.y,
            );
            f.set_cursor_position(cursor_pos);

            if !self.state.search_results.is_empty() {
                let items: Vec<ListItem> = self
                    .state
                    .search_results
                    .iter()
                    .map(|obs| {
                        let truncated = if obs.content.len() > 60 {
                            format!("{}...", &obs.content[..60])
                        } else {
                            obs.content.clone()
                        };
                        let content = format!(
                            "[{}] {} | {}",
                            obs.created_at.0, obs.observation_type, truncated
                        );
                        ListItem::new(content)
                    })
                    .collect();

                let mut list_state = ListState::default();
                list_state.select(Some(self.state.selected_index));

                let list = List::new(items)
                    .block(Block::default().title(" Results ").borders(Borders::ALL))
                    .highlight_style(Style::new().bg(Color::Blue).fg(Color::White))
                    .highlight_symbol(">> ");

                f.render_stateful_widget(list, chunks[1], &mut list_state);
            } else if !self.state.search_query.is_empty() {
                let text = Paragraph::new("No results found.").style(Style::new().fg(Color::Gray));
                f.render_widget(text, chunks[1]);
            }
        }

        fn render_sessions(&self, f: &mut Frame, area: Rect) {
            if self.state.sessions.is_empty() {
                let text = Paragraph::new("No sessions found.").style(Style::new().fg(Color::Gray));
                f.render_widget(text, area);
                return;
            }

            let items: Vec<ListItem> = self
                .state
                .sessions
                .iter()
                .map(|session| {
                    let content = format!(
                        "[{}] {} | {} | {} obs",
                        session.started_at.0,
                        session.project,
                        session.id.as_str(),
                        session.observation_count
                    );
                    ListItem::new(content)
                })
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(self.state.selected_index));

            let list = List::new(items)
                .block(Block::default().title(" Sessions ").borders(Borders::ALL))
                .highlight_style(Style::new().bg(Color::Blue).fg(Color::White))
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, area, &mut list_state);
        }

        fn render_stats(&self, f: &mut Frame, area: Rect) {
            if let Some(stats) = &self.state.stats {
                let obs_count = stats["total_observations"].as_i64().unwrap_or(0).to_string();
                let sess_count = stats["total_sessions"].as_i64().unwrap_or(0).to_string();
                let storage_size = format!("{} bytes", stats["storage_size_bytes"].as_i64().unwrap_or(0));

                let rows = [
                    Row::new(["Total Observations", obs_count.as_str()]),
                    Row::new(["Total Sessions", sess_count.as_str()]),
                    Row::new(["Storage Size", storage_size.as_str()]),
                ];

                let table = Table::new(
                    rows,
                    &[Constraint::Percentage(50), Constraint::Percentage(50)],
                )
                .block(Block::default().title(" Statistics ").borders(Borders::ALL))
                .style(Style::new().fg(Color::White));

                f.render_widget(table, area);
            } else {
                let text = Paragraph::new("Press 'r' to refresh stats.")
                    .style(Style::new().fg(Color::Gray));
                f.render_widget(text, area);
            }
        }

        fn render_confirm_quit(&self, f: &mut Frame, area: Rect) {
            let block = Block::default()
                .title(" Confirm Quit ")
                .borders(Borders::ALL)
                .style(Style::new().bg(Color::Black));

            f.render_widget(block, area);

            let text = Paragraph::new("Are you sure you want to quit? [y/N]")
                .style(Style::new().fg(Color::Yellow));

            let inner = Rect {
                x: area.x + 2,
                y: area.y + 1,
                width: area.width.saturating_sub(4),
                height: 1,
            };
            f.render_widget(text, inner);
        }

        fn render_footer(&self, f: &mut Frame, area: Rect) {
            let msg = if let Some(ref m) = self.state.message {
                m.clone()
            } else {
                format!(
                    "{} observations | Mode: {:?}",
                    self.state.observations.len(),
                    self.state.mode
                )
            };

            let text = Paragraph::new(msg).style(Style::new().fg(Color::DarkGray));

            f.render_widget(text, area);
        }
    }
}
