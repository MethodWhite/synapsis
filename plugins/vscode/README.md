# Synapsis + VS Code / Cursor / Windsurf

## Instalación automática (recomendada)

```bash
synapsis-autoconfig --apply
```

Esto detecta qué IDE tienes instalado y genera la configuración MCP.

## Instalación manual

```bash
# VS Code
cp mcp-settings.json ~/.vscode/mcp.json

# Cursor
cp mcp-settings.json ~/.cursor/mcp.json

# Windsurf
cp mcp-settings.json ~/.windsurf/mcp.json
```

## Uso

Los MCP tools de Synapsis estarán disponibles en el IDE:
- `mem_save` / `mem_search`
- `shared_sessions_list` / `shared_sessions_broadcast`
- `discovery_scan`
