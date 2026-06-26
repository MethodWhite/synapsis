//! Synapsis Core Module

pub mod agent;
pub mod antibrick;
pub mod auth;
pub mod auto_integrate;
pub mod discovery;
pub mod discovery_net;
pub mod lock_utils;
pub mod orchestrator;
pub mod pqc;
pub mod rate_limiter;
pub mod recycle;
pub mod resource_manager;
pub mod retry;
pub mod security;
pub mod sync;
pub mod task_queue;
pub mod tool_registry;
pub mod uuid;
pub mod vault;
pub mod watchdog;
pub mod worker;

pub use agent::*;
pub use auth::*;
pub use auto_integrate::*;
pub use discovery::*;
pub use orchestrator::{
    Agent, AgentStatus, LegacyFile, MessageType, Orchestrator, OrchestratorMessage, ReviewStatus,
    Task as OrchestratorTask, TaskStatus as OrchestratorTaskStatus,
};
pub use pqc::*;
pub use rate_limiter::*;
pub use recycle::*;
pub use retry::*;
pub use security::*;
pub use sync::*;
pub use task_queue::*;
pub use tool_registry::*;
pub use uuid::*;
pub use vault::*;
pub use worker::{
    CodeWorker, FileWorker, GitWorker, OpenCodeConnector, QwenConnector, SearchWorker, ShellWorker,
    Task as WorkerTask, TaskStatus as WorkerTaskStatus, WorkerAgent, WorkerRegistry,
};
pub mod agent_registry_ext;
pub mod audit_log;
pub mod chunk_query;
pub mod providers;
pub mod session_id;
pub mod session_manager;
pub mod task_cleanup;
pub mod timeline_manager;
