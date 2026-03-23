# Synapsis MCP for Claude Code

Integration plugin for Claude Code CLI with Synapsis MCP server.

## Installation

```bash
# Copy to Claude Code config directory
cp synapsis-mcp.json ~/.claude/
cp synapsis-session-start.sh ~/.claude/hooks/
chmod +x ~/.claude/hooks/synapsis-session-start.sh
```

## Configuration

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "synapsis": {
      "command": "synapsis",
      "args": ["mcp"],
      "env": {
        "SYNAPSIS_PROJECT": "claude-code-project"
      }
    }
  }
}
```

## Features

- ✅ Auto-register session on start
- ✅ Heartbeat every 30 seconds
- ✅ Save context after commands
- ✅ Search memory before coding
- ✅ Distributed locks for builds
- ✅ Multi-agent coordination

## Usage

```bash
# Start Claude Code with Synapsis
claude

# Synapsis auto-registers and starts heartbeat
# Context is saved automatically
```

## Commands

| Command | Description |
|---------|-------------|
| `/synapsis save <title>` | Save current context |
| `/synapsis search <query>` | Search memory |
| `/synapsis context` | Get global context |
| `/synapsis agents` | List active agents |
