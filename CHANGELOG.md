# Changelog

All notable changes to Synapsis will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-05-03

### Fixed
- **Critical**: PersistentEventBus fully implemented with SQLite-backed events table
  - Inter-agent messaging now works (CLI <-> IDE <-> TUI)
  - Direct messages (`send_message`/`get_pending_messages`) between agents
  - Broadcast to channels with polling (`broadcast`/`event_poll`)
  - Event acknowledgment (`event_ack`)
  - Automatic cleanup of expired events
- **Critical**: `McpServer::init()` now calls `db.init()` - tables created on fresh install
- **Critical**: `mem_lock_acquire`/`mem_lock_release` accept both `resource`/`ttl_seconds` (MCP spec) and legacy `lock_key`/`ttl_secs` params
- **Major**: `mw-cli` SynapsisMcpClient rewritten with real JSON-RPC 2.0 MCP stdio communication
  - Full handshake: `initialize` -> `initialized` -> `mem_session_start`
  - All operations use real MCP tool calls instead of mock stubs

### Added
- `events` table in SQLite with indexes for efficient agent/channel/project filtering
- Database methods: `publish_event`, `broadcast_event`, `poll_events`, `get_pending_messages`, `acknowledge_event`, `cleanup_expired_events`
- `connection_status` MCP tool to see active CLI/IDE/TUI connections

## [0.2.0] - 2026-04-02

### Added
- MCP Server Hardening & Installer Infrastructure

## [0.1.0] - 2026-03-22

### Initial Release

- Persistent memory engine with SQLite + FTS5
- MCP server implementation
- TCP server for multi-agent coordination
- PQC security primitives (CRYSTALS-Kyber, CRYSTALS-Dilithium)
- Zero-trust architecture
- Session management with auto-reconnect
- Distributed locks
- Task queue

---

**Security Score:** 8.5/10  
**Last Updated:** 2026-03-22
