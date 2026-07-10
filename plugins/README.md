# Synapsis MCP Plugins

Synapsis es un **MCP Server** nativo. Los plugins en este directorio son
configuraciones para que clientes MCP (CLIs, IDEs) se conecten a Synapsis.

No son plugins del servidor — son **configs de clientes** que apuntan al binario
`synapsis-mcp` como MCP server.

## Plugins disponibles

| Cliente | Archivo | Descripción |
|---------|---------|-------------|
| OpenCode | `opencode.jsonc` | Config para OpenCode CLI |
| Claude Code | `claude-code/` | Config + session hooks |
| VS Code / Cursor / Windsurf | `vscode/` | MCP configs para IDEs |
| JetBrains | `jetbrains/` | Plugin para IDEs JetBrains |
| Gemini CLI | `gemini-cli/` | Script de conexión |

## Auto-configuración

```bash
synapsis-autoconfig --apply
```

Esto detecta automáticamente qué clientes están instalados y genera
las configuraciones MCP necesarias.
