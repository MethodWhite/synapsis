//! Synapsis Presentation Layer

pub mod cli;
pub mod http;
pub mod mcp;
pub mod tui;

pub use cli::CLI;
pub use http::HTTPServer;
pub use mcp::McpServer;
pub use tui::{Tui, TuiCommand};
