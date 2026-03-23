# Synapsis MCP Plugin for JetBrains IDEs

Plugin MCP para IntelliJ IDEA, WebStorm, PyCharm, RustRover, etc.

## Instalación

1. Copiar `mcp-config.json` a tu directorio de configuración de JetBrains
2. Reiniciar el IDE
3. Habilitar plugin en Settings → Tools → Synapsis MCP

## Configuración

```json
{
  "synapsis": {
    "enabled": true,
    "server": "tcp://127.0.0.1:7438",
    "project_key": "${project_name}",
    "auto_heartbeat": true,
    "heartbeat_interval": 30
  }
}
```

## Comandos Disponibles

- `Synapsis: Save Context` - Guardar contexto actual
- `Synapsis: Search` - Buscar en memoria
- `Synapsis: Global Context` - Ver reglas del proyecto
- `Synapsis: Active Agents` - Ver agentes activos

## Features

- ✅ Auto-registro de sesión
- ✅ Heartbeat automático
- ✅ Contexto por proyecto
- ✅ Búsqueda FTS5
- ✅ Locks distribuidos para builds
