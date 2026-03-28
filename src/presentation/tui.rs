//! Synapsis TUI - Minimal terminal UI

use crate::SynapsisError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use synapsis_core::core::orchestrator::Orchestrator;
use synapsis_core::domain::{
    entities::{Observation, SearchParams, SessionSummary},
    ports::{SessionPort, StoragePort},
};

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
    pub orchestrator: Option<Arc<Orchestrator>>,
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
    pub scroll_offset: usize,
    pub input_buffer: String,
    pub message: Option<String>,
    pub stats: Option<TuiStats>,
    pub agents: Vec<AgentInfo>,
    pub tasks: Vec<TaskInfo>,
    pub connection_status: ConnectionStatus,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionStatus {
    pub mcp_connected: bool,
    pub tcp_connected: bool,
    pub active_agents: usize,
    pub pending_tasks: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppMode {
    #[default]
    Timeline,
    AddObservation,
    Search,
    Sessions,
    Stats,
    Agents,
    Tasks,
    Connection,
    ConfirmQuit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiStats {
    pub total_observations: usize,
    pub total_sessions: usize,
    pub storage_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_type: String,
    pub session_id: String,
    pub project: String,
    pub status: String,
    pub last_heartbeat: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    pub assigned_to: Option<String>,
    pub status: String,
    pub created_at: i64,
}

impl Tui {
    pub fn new(
        storage: Arc<dyn StoragePort>,
        sessions: Arc<dyn SessionPort>,
        orchestrator: Option<Arc<Orchestrator>>,
    ) -> Self {
        Self {
            storage,
            sessions,
            orchestrator,
            state: TuiState::default(),
        }
    }

    #[cfg(not(feature = "tui"))]
    pub fn run(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        Err(Box::new(SynapsisError::internal_unimplemented()))
    }

    fn refresh_data(&mut self) -> synapsis_core::domain::errors::Result<()> {
        let entries = self.storage.get_timeline(1000)?;
        self.state.observations = entries.into_iter().map(|e| e.observation).collect();
        self.state.selected_index = 0;
        Ok(())
    }

    fn perform_search(&mut self) -> synapsis_core::domain::errors::Result<()> {
        let params = SearchParams::new(&self.state.search_query).with_limit(50);
        let results = self.storage.search_observations(&params)?;
        self.state.search_results = results.into_iter().map(|r| r.observation).collect();
        self.state.selected_index = 0;
        Ok(())
    }

    fn calculate_stats(&mut self) -> synapsis_core::domain::errors::Result<()> {
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

    pub fn update_connection_status(&mut self, mcp: bool, tcp: bool, agents: usize, tasks: usize) {
        self.state.connection_status = ConnectionStatus {
            mcp_connected: mcp,
            tcp_connected: tcp,
            active_agents: agents,
            pending_tasks: tasks,
        };
    }

    pub fn refresh_agents_from_orchestrator(&mut self) {
        if let Some(orch) = &self.orchestrator {
            let status = orch.get_system_status();
            if let Some(agents_array) = status.get("agents").and_then(|a| a.as_array()) {
                self.state.agents = agents_array
                    .iter()
                    .filter_map(|a| {
                        Some(AgentInfo {
                            agent_type: a.get("agent_type")?.as_str()?.to_string(),
                            session_id: a.get("id")?.as_str()?.to_string(),
                            project: a
                                .get("project")
                                .and_then(|p| p.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            status: format!(
                                "{:?}",
                                a.get("status")
                                    .and_then(|s| s.get("Idle"))
                                    .unwrap_or(&serde_json::json!("Idle"))
                            ),
                            last_heartbeat: a.get("last_heartbeat")?.as_i64()?,
                        })
                    })
                    .collect();
            }
        }
        self.state.selected_index = 0;
    }

    pub fn refresh_tasks_from_orchestrator(&mut self) {
        if let Some(orch) = &self.orchestrator {
            let pending = orch.get_pending_tasks();
            self.state.tasks = pending
                .into_iter()
                .map(|t| TaskInfo {
                    id: t.id,
                    title: t.description,
                    assigned_to: t.assigned_to,
                    status: format!("{:?}", t.status),
                    created_at: t.created_at,
                })
                .collect();
        }
        self.state.selected_index = 0;
    }

    pub fn update_connection_from_orchestrator(&mut self) {
        if let Some(orch) = &self.orchestrator {
            let status = orch.get_system_status();
            let active_agents = status
                .get("active_agents")
                .and_then(|a| a.as_u64())
                .unwrap_or(0) as usize;
            let pending_tasks = orch.get_pending_tasks().len();

            self.state.connection_status = ConnectionStatus {
                mcp_connected: true,
                tcp_connected: true,
                active_agents,
                pending_tasks,
            };
        }
    }

    pub fn refresh_agents(&mut self, agents: Vec<AgentInfo>) {
        self.state.agents = agents;
        self.state.selected_index = 0;
    }

    pub fn refresh_tasks(&mut self, tasks: Vec<TaskInfo>) {
        self.state.tasks = tasks;
        self.state.selected_index = 0;
    }
}

#[cfg(feature = "tui")]
mod tui_impl {
    use super::*;
    use crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEventKind, KeyModifiers,
        },
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
    use std::io::Write;

    impl Tui {
        pub fn run(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
            enable_raw_mode()?;
            let mut stdout = io::stdout();

            // Set terminal window title for Hyprland window rules (ANSI escape sequence)
            print!("\x1b]2;Synapsis TUI\x07");
            io::stdout().flush().unwrap_or(());

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
        ) -> synapsis_core::domain::errors::Result<()> {
            self.refresh_data()?;

            loop {
                terminal.draw(|f| self.render(f))?;

                if let event::Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
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
                                KeyCode::Char('g') => {
                                    self.state.mode = AppMode::Agents;
                                }
                                KeyCode::Char('T') => {
                                    self.state.mode = AppMode::Tasks;
                                }
                                KeyCode::Char('c') => {
                                    self.state.mode = AppMode::Connection;
                                }
                                KeyCode::Char('G') => {
                                    if !self.state.observations.is_empty() {
                                        self.state.selected_index =
                                            self.state.observations.len() - 1;
                                    }
                                }
                                KeyCode::Char('0') => {
                                    self.state.selected_index = 0;
                                }
                                KeyCode::Char('d') if ctrl => {
                                    let max = self.state.observations.len().saturating_sub(1);
                                    self.state.selected_index =
                                        (self.state.selected_index + 10).min(max);
                                }
                                KeyCode::Char('u') if ctrl => {
                                    self.state.selected_index =
                                        self.state.selected_index.saturating_sub(10);
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
                            AppMode::Agents => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.state.mode = AppMode::Timeline;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if self.state.selected_index > 0 {
                                        self.state.selected_index -= 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let max = self.state.agents.len().saturating_sub(1);
                                    if self.state.selected_index < max {
                                        self.state.selected_index += 1;
                                    }
                                }
                                KeyCode::Char('r') => {
                                    self.refresh_agents_from_orchestrator();
                                    self.state.message = Some(format!(
                                        "Refreshed {} agents",
                                        self.state.agents.len()
                                    ));
                                }
                                _ => {}
                            },
                            AppMode::Tasks => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.state.mode = AppMode::Timeline;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if self.state.selected_index > 0 {
                                        self.state.selected_index -= 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let max = self.state.tasks.len().saturating_sub(1);
                                    if self.state.selected_index < max {
                                        self.state.selected_index += 1;
                                    }
                                }
                                KeyCode::Char('r') => {
                                    self.refresh_tasks_from_orchestrator();
                                    self.state.message =
                                        Some(format!("Refreshed {} tasks", self.state.tasks.len()));
                                }
                                _ => {}
                            },
                            AppMode::Connection => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.state.mode = AppMode::Timeline;
                                }
                                KeyCode::Char('r') => {
                                    self.update_connection_from_orchestrator();
                                    self.state.message =
                                        Some("Connection status refreshed".to_string());
                                }
                                _ => {}
                            },
                            AppMode::Tasks => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.state.mode = AppMode::Timeline;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if self.state.selected_index > 0 {
                                        self.state.selected_index -= 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let max = self.state.tasks.len().saturating_sub(1);
                                    if self.state.selected_index < max {
                                        self.state.selected_index += 1;
                                    }
                                }
                                KeyCode::Char('r') => {
                                    self.state.message =
                                        Some("Task refresh not implemented yet".to_string());
                                }
                                _ => {}
                            },
                            AppMode::Connection => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    self.state.mode = AppMode::Timeline;
                                }
                                KeyCode::Char('r') => {
                                    self.state.message =
                                        Some("Connection refresh not implemented yet".to_string());
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
                AppMode::Agents => self.render_agents(f, chunks[1]),
                AppMode::Tasks => self.render_tasks(f, chunks[1]),
                AppMode::Connection => self.render_connection(f, chunks[1]),
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
                AppMode::Agents => "Synapsis - Active Agents",
                AppMode::Tasks => "Synapsis - Tasks",
                AppMode::Connection => "Synapsis - Connection Status",
                AppMode::ConfirmQuit => "Synapsis - Confirm Quit",
            };

            let help_text = match self.state.mode {
                AppMode::Timeline => {
                    "[a]dd  [s]earch  [l]essions  [S]tats  [g]ents  [T]asks  [c]onn  [t]refresh  [q]uit"
                }
                AppMode::AddObservation => "[Enter] save  [Esc] cancel",
                AppMode::Search => "[Enter] search  [Esc] cancel",
                AppMode::Sessions => "[k/j] navigate  [r] refresh  [q/Esc] back",
                AppMode::Stats => "[r] refresh  [q/Esc] back",
                AppMode::Agents => "[r] refresh  [q/Esc] back",
                AppMode::Tasks => "[r] refresh  [q/Esc] back",
                AppMode::Connection => "[r] refresh  [q/Esc] back",
                AppMode::ConfirmQuit => "[y] yes  [n] no",
            };

            let block = Block::default()
                .title(format!(" {} ", title))
                .title_style(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL);

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
                .borders(Borders::ALL);

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
                let obs_count = stats.total_observations.to_string();
                let sess_count = stats.total_sessions.to_string();
                let storage_size = format!("{} bytes", stats.storage_size_bytes);

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

        fn render_agents(&self, f: &mut Frame, area: Rect) {
            if self.state.agents.is_empty() {
                let text = Paragraph::new(
                    "No active agents. Press 'r' to refresh or use MCP to connect agents.",
                )
                .style(Style::new().fg(Color::Gray));
                f.render_widget(text, area);
                return;
            }

            let items: Vec<ListItem> = self
                .state
                .agents
                .iter()
                .map(|agent| {
                    let status_color = if agent.status == "busy" {
                        "🔴"
                    } else {
                        "🟢"
                    };
                    let content = format!(
                        "{} [{}] {} | {} | {}",
                        status_color,
                        agent.agent_type,
                        agent.session_id.split('-').next().unwrap_or("?"),
                        agent.project,
                        agent.last_heartbeat
                    );
                    ListItem::new(content)
                })
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(self.state.selected_index));

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(" Active Agents ")
                        .borders(Borders::ALL),
                )
                .highlight_style(Style::new().bg(Color::Blue).fg(Color::White))
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, area, &mut list_state);
        }

        fn render_tasks(&self, f: &mut Frame, area: Rect) {
            if self.state.tasks.is_empty() {
                let text = Paragraph::new("No pending tasks. Press 'r' to refresh.")
                    .style(Style::new().fg(Color::Gray));
                f.render_widget(text, area);
                return;
            }

            let items: Vec<ListItem> = self
                .state
                .tasks
                .iter()
                .map(|task| {
                    let status_symbol = match task.status.as_str() {
                        "pending" => "⏳",
                        "in_progress" => "🔄",
                        "completed" => "✅",
                        "failed" => "❌",
                        _ => "❓",
                    };
                    let content = format!(
                        "{} [{}] {} | {}",
                        status_symbol,
                        task.status,
                        task.title,
                        task.assigned_to.as_deref().unwrap_or("unassigned")
                    );
                    ListItem::new(content)
                })
                .collect();

            let mut list_state = ListState::default();
            list_state.select(Some(self.state.selected_index));

            let list = List::new(items)
                .block(Block::default().title(" Tasks ").borders(Borders::ALL))
                .highlight_style(Style::new().bg(Color::Blue).fg(Color::White))
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, area, &mut list_state);
        }

        fn render_connection(&self, f: &mut Frame, area: Rect) {
            let status = &self.state.connection_status;

            let mcp_status = if status.mcp_connected {
                "🟢 Connected"
            } else {
                "🔴 Disconnected"
            };
            let tcp_status = if status.tcp_connected {
                "🟢 Connected"
            } else {
                "🔴 Disconnected"
            };
            let agents_count = status.active_agents.to_string();
            let tasks_count = status.pending_tasks.to_string();

            let rows = [
                Row::new(["MCP Server", mcp_status]),
                Row::new(["TCP Server", tcp_status]),
                Row::new(["Active Agents", &agents_count]),
                Row::new(["Pending Tasks", &tasks_count]),
            ];

            let table = Table::new(
                rows,
                &[Constraint::Percentage(40), Constraint::Percentage(60)],
            )
            .block(
                Block::default()
                    .title(" Connection Status ")
                    .borders(Borders::ALL),
            )
            .style(Style::new().fg(Color::White));

            f.render_widget(table, area);
        }

        fn render_confirm_quit(&self, f: &mut Frame, area: Rect) {
            let block = Block::default()
                .title(" Confirm Quit ")
                .borders(Borders::ALL);

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

            let text = Paragraph::new(msg).style(Style::new().fg(Color::Gray));

            f.render_widget(text, area);
        }
    }
}
