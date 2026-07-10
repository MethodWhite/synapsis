# Synapsis + Claude Code

## Instalación

```bash
cp mcp-config.json ~/.claude/mcp.json
# o
synapsis-autoconfig --apply
```

## Uso

Claude Code puede usar los MCP tools de Synapsis para:
- `mem_save` — guardar observaciones importantes
- `mem_search` — buscar en memoria persistente
- `mem_context` — obtener contexto de sesiones previas
- `shared_sessions_list` — ver sesiones activas de otras herramientas

## Session hooks

`synapsis-session-start.sh` se ejecuta automáticamente cuando Claude Code
inicia una sesión, registrándola en el Session Bridge.
