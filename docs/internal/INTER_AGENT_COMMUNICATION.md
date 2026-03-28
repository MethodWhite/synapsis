# Synapsis EventBus - Inter-Agent Communication System

## Overview

El sistema **EventBus Persistente** de Synapsis permite la comunicación en tiempo real entre múltiples agentes, IDEs, CLIs y TUIs conectados simultáneamente. Todos los agentes pueden compartir contexto, coordinar tareas y comunicarse entre sí mediante un sistema de mensajería basado en SQLite.

## Arquitectura

```
┌─────────────────────────────────────────────────────────────────┐
│              SQLite Database (synapsis.db)                      │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Table: events                                           │  │
│  │  - Broadcast messages (channel-based)                    │  │
│  │  - Direct messages (session-to-session)                  │  │
│  │  - System events (task, agent status)                    │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
         ▲                    ▲                    ▲
         │                    │                    │
   ┌─────┴─────┐        ┌─────┴─────┐        ┌─────┴─────┐
   │ Qwen Code │        │ Cursor IDE│        │  CLI/TUI  │
   │  (MCP)    │        │   (MCP)   │        │  (comm)   │
   └───────────┘        └───────────┘        └───────────┘
```

## Características

### ✅ Implementadas

- **Broadcast Messaging**: Envía mensajes a todos los agentes en un canal
- **Direct Messaging**: Envía mensajes punto-a-punto entre sesiones
- **Channel System**: Canales temáticos para organizar comunicación
- **Priority Levels**: Prioridades (0=normal, 1=high, 2=critical)
- **Event Polling**: Recupera eventos nuevos eficientemente
- **Persistent Storage**: Todos los eventos se guardan en SQLite
- **Multi-Project**: Soporte para múltiples proyectos simultáneos
- **CLI Tool**: `synapsis comm` para comunicación desde terminal

### 🔄 En Progreso

- Push notifications vía SQLite WAL
- WebSocket support para actualizaciones en tiempo real
- Message expiration/auto-cleanup

## Métodos MCP Disponibles

### 1. `broadcast` - Enviar mensaje a todos los agentes

Envía un mensaje a **TODOS** los agentes conectados en un canal específico.

**Parámetros:**
```json
{
  "session_id": "tu-session-id",
  "content": "Mensaje o JSON con datos estructurados",
  "channel": "global",        // opcional, default: "global"
  "project": "my-project",    // opcional
  "priority": 0,              // 0=normal, 1=high, 2=critical
  "type": "broadcast"         // tipo de evento
}
```

**Ejemplo de uso desde MCP:**
```json
{"jsonrpc":"2.0","id":1,"method":"broadcast","params":{
  "session_id": "qwen-session-123",
  "content": "{\"type\":\"status\",\"task\":\"deploy\",\"status\":\"starting\"}",
  "channel": "deploy",
  "priority": 1
}}
```

**Respuesta:**
```json
{"jsonrpc":"2.0","id":1,"result":{
  "content": [{"type":"text","text":"Broadcast sent to channel 'deploy' (event_id: 42)"}]
}}
```

### 2. `event_poll` - Recuperar eventos nuevos

Recupera todos los eventos creados después de un timestamp específico.

**Parámetros:**
```json
{
  "since": 1774384000,      // Unix timestamp (requerido)
  "channel": "global",      // opcional - filtra por canal
  "project": "my-project",  // opcional - filtra por proyecto
  "limit": 100              // opcional, default: 100
}
```

**Ejemplo:**
```json
{"jsonrpc":"2.0","id":2,"method":"event_poll","params":{
  "since": 1774384000,
  "channel": "global",
  "limit": 50
}}
```

**Respuesta:**
```json
{"jsonrpc":"2.0","id":2,"result":{
  "events": [
    {
      "id": 42,
      "event_type": "broadcast",
      "from": "qwen-session-123",
      "to": null,
      "project": "default",
      "channel": "global",
      "content": "{\"type\":\"status\",\"task\":\"deploy\"}",
      "priority": 1,
      "timestamp": 1774384050
    }
  ],
  "count": 1
}}
```

### 3. `send_message` - Mensaje directo entre agentes

Envía un mensaje directo de una sesión a otra.

**Parámetros:**
```json
{
  "session_id": "tu-session-id",
  "to": "receiver-session-id",
  "content": "Mensaje directo",
  "project": "default"  // opcional
}
```

### 4. `get_pending_messages` - Obtener mensajes pendientes

Recupera mensajes directos no leídos para una sesión.

**Parámetros:**
```json
{
  "session_id": "tu-session-id"
}
```

### 5. `event_ack` - Marcar evento como leído

Confirma la recepción de un evento.

**Parámetros:**
```json
{
  "event_id": 42
}
```

### 6. `agents_active` - Ver agentes activos

Lista todos los agentes activos en el sistema.

**Parámetros:**
```json
{
  "project": "default"  // opcional
}
```

## CLI: `synapsis comm`

Herramienta de línea de comandos para comunicación inter-agente.

### Instalación

```bash
cd /home/methodwhite/Projects/synapsis
cargo build --release --bin comm
```

El binario se encuentra en: `target/release/comm`

### Comandos

#### 1. Broadcast - Enviar mensaje a todos

```bash
# Enviar mensaje al canal global
synapsis comm broadcast "Hola a todos!"

# Enviar con prioridad alta al canal "deploy"
synapsis comm broadcast "⚠️ Deploy iniciado" --channel deploy --priority 1

# Enviar a un proyecto específico
synapsis comm broadcast "Update completo" --project my-project
```

**Opciones:**
- `--channel, -c <channel>`: Canal de destino (default: "global")
- `--project, -p <project>`: Proyecto específico
- `--priority, -P <0-2>`: Prioridad (0=normal, 1=high, 2=critical)

#### 2. Poll - Recuperar eventos

```bash
# Recuperar todos los eventos desde el inicio
synapsis comm poll --since 0

# Recuperar eventos de un canal específico
synapsis comm poll --channel deploy --since 1774384000

# Limitar resultados
synapsis comm poll --limit 10 --since 0
```

**Opciones:**
- `--since, -s <timestamp>`: Unix timestamp (requerido)
- `--channel, -c <channel>`: Filtrar por canal
- `--project, -p <project>`: Filtrar por proyecto
- `--limit, -l <n>`: Máximo de eventos (default: 20)

#### 3. Agents - Ver agentes activos

```bash
# Ver todos los agentes
synapsis comm agents

# Ver agentes de un proyecto
synapsis comm agents --project default
```

### Ejemplos de Uso

#### Coordinar Deploy entre Múltiples Agentes

```bash
# Agente 1 (Qwen): Iniciar deploy
synapsis comm broadcast "🚀 Iniciando deploy a producción" \
  --channel deploy --priority 1

# Agente 2 (Cursor): Confirmar
synapsis comm broadcast "✅ Tests pasados, listo para deploy" \
  --channel deploy

# Agente 3 (CLI): Monitorear eventos
synapsis comm poll --channel deploy --since $(date +%s)
```

#### Sistema de Notificaciones

```bash
# Enviar notificación crítica
synapsis comm broadcast "🔴 ERROR: Database connection lost" \
  --channel alerts --priority 2

# Suscribirse a alertas (poll continuo)
while true; do
  synapsis comm poll --channel alerts --since $LAST_TS
  sleep 5
done
```

## Schema de Base de Datos

```sql
CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,        -- 'broadcast', 'message', 'task', etc.
    from_session_id TEXT NOT NULL,   -- Sesión que envía
    to_session_id TEXT,              -- NULL = broadcast, SessionID = directo
    project TEXT,                    -- Proyecto asociado
    channel TEXT DEFAULT 'global',   -- Canal temático
    content TEXT NOT NULL,           -- JSON con payload
    priority INTEGER DEFAULT 0,      -- 0=normal, 1=high, 2=critical
    read_at INTEGER,                 -- Timestamp de lectura (NULL = no leído)
    created_at INTEGER NOT NULL      -- Timestamp de creación
);

-- Índices para performance
CREATE INDEX idx_events_to_unread ON events(to_session_id, read_at) WHERE read_at IS NULL;
CREATE INDEX idx_events_channel ON events(channel, created_at);
CREATE INDEX idx_events_project ON events(project, created_at);
CREATE INDEX idx_events_created ON events(created_at);
```

## Patrones de Uso Recomendados

### 1. Polling Eficiente

Cada agente debe mantener un `last_event_id` y hacer poll cada 500ms-1s:

```javascript
let lastEventId = 0;

setInterval(async () => {
  const result = await mcpCall('event_poll', { since: lastEventId });
  
  for (const event of result.events) {
    handleEvent(event);
    lastEventId = Math.max(lastEventId, event.id);
  }
}, 1000);
```

### 2. Contenido Estructurado

Usa JSON para el contenido de los mensajes:

```json
{
  "type": "coordination_request",
  "from_agent": "qwen-code",
  "task": "database_migration",
  "status": "seeking_help",
  "details": {
    "description": "Need help with SQL optimization",
    "priority": "high"
  }
}
```

### 3. Canales Temáticos

Organiza la comunicación por canales:

- `global`: Mensajes generales
- `deploy`: Coordinación de deploys
- `alerts`: Alertas críticas
- `tasks`: Coordinación de tareas
- `audit`: Auditoría y logs

### 4. Limpieza Automática

Limpia eventos antiguos periódicamente:

```bash
# En cron cada hora
0 * * * * /path/to/synapsis comm cleanup --older-than 86400
```

## Casos de Uso

### 1. Coordinación Multi-Agente

Varios agentes trabajando en la misma tarea:

```
[Agent A] → broadcast: "Starting task X"
[Agent B] → broadcast: "I'll handle subtask X.1"
[Agent C] → broadcast: "Working on subtask X.2"
[Agent A] → broadcast: "Task X complete"
```

### 2. Notificaciones en Tiempo Real

Sistema de alertas distribuido:

```
[Monitor Agent] → broadcast(priority=2): "🔴 CPU usage > 90%"
[All Agents] → poll(channel="alerts") → receive alert
[Action Agent] → broadcast: "Scaling up resources"
```

### 3. Compartir Contexto de Sesión

Agentes compartiendo descubrimientos:

```
[Qwen] → broadcast: {
  "type": "discovery",
  "finding": "Found security vulnerability in auth module",
  "severity": "high",
  "location": "src/auth.rs:142"
}

[Cursor] → poll() → receives discovery
[Cursor] → broadcast: {
  "type": "fix_started",
  "referencing": "discovery:42",
  "eta": "5 minutes"
}
```

## Troubleshooting

### Error: "SQL error or missing database"

**Causa:** La tabla `events` no existe.

**Solución:**
```bash
sqlite3 ~/.local/share/synapsis/synapsis.db <<EOF
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    from_session_id TEXT NOT NULL,
    to_session_id TEXT,
    project TEXT,
    channel TEXT DEFAULT 'global',
    content TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    read_at INTEGER,
    created_at INTEGER NOT NULL
);
EOF
```

### Error: "No new events" pero sé que hay eventos

**Causa:** El timestamp `since` es muy reciente.

**Solución:** Usa `since: 0` para ver todos los eventos:
```bash
synapsis comm poll --since 0
```

### Agentes no reciben broadcasts

**Causa:** No están haciendo poll regularmente.

**Solución:** Implementa polling cada 1-2 segundos en el agente.

## Próximas Mejoras

1. **Push Notifications**: Usar SQLite WAL hooks para notificaciones push
2. **WebSocket Gateway**: Gateway WebSocket para clientes web
3. **Message Expiration**: Auto-limpieza de eventos antiguos
4. **Encryption**: Cifrado de mensajes sensibles
5. **Delivery Confirmation**: Confirmación de entrega garantizada
6. **Message Routing**: Enrutamiento inteligente por tipo de agente

## Referencias

- MCP Server: `/home/methodwhite/Projects/synapsis/src/presentation/mcp/server.rs`
- Database Module: `/home/methodwhite/Projects/synapsis-core/src/infrastructure/database/mod.rs`
- CLI Tool: `/home/methodwhite/Projects/synapsis/src/bin/comm.rs`

---

**Estado:** ✅ Implementado y en Producción
**Versión:** 1.0.0
**Última Actualización:** 2026-03-24
