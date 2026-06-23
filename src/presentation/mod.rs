//! Synapsis Presentation Layer

pub mod cli;
pub mod http;
pub mod mcp;
pub mod quic;
pub mod tui;

pub use cli::CLI;
pub use http::HttpTransport;
pub use mcp::McpServer;
pub use quic::QuicTransport;
pub use tui::{Tui, TuiCommand};
