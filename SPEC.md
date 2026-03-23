# Synapsis - Sistema de Memoria Persistente para Agentes IA

## Visión

**Synapsis** es un motor de memoria persistente de nivel militar para agentes IA, escrito en Rust puro desde cero. Combina criptografía post-cuántica (PQC) con arquitectura zero-trust para crear el sistema de memoria más seguro y eficiente disponible.

**Diferenciador clave**: Diseñado desde cero para uso **multi-agente** con garantías de consistencia.

---

## 10-Star Security Model

### Implementación de Seguridad

| Nivel | Componente | Tecnología |
|-------|------------|------------|
| ⭐ | Criptografía PQC | CRYSTALS-Kyber-1024 (KEM), CRYSTALS-Dilithium-4 (Firma) |
| ⭐⭐ | Zero-Trust | Verificación continua, mínimo privilegio, microsegmentación |
| ⭐⭐⭐ | Integridad | HMAC-SHA3-512, Merkle Trees para verificación |
| ⭐⭐⭐⭐ | Confidencialidad | Cifrado AES-256-GCM + ChaCha20-Poly1305 |
| ⭐⭐⭐⭐⭐ | Autenticación | Firmas PQC en cada operación |
| ⭐⭐⭐⭐⭐⭐ | Non-repudio | Log inmutable con timestamps criptográficos |
| ⭐⭐⭐⭐⭐⭐⭐ | Resiliencia | Redundancia, backups verificables, recovery |
| ⭐⭐⭐⭐⭐⭐⭐⭐ | Auditoría | every operation logged con audit trail |
| ⭐⭐⭐⭐⭐⭐⭐⭐⭐ | Anti-tampering | Detección de manipulación, alertas automáticas |
| ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ | Auto-reparación | Recovery automático ante ataques |

---

## Multi-Agent Architecture (CRÍTICO)

### Problemas de Engram Corregidos

| # | Problema Engram | Gravedad | Solución Synapsis |
|---|----------------|----------|-------------------|
| 1 | **Race condition en saves concurrentes** | CRÍTICA | `INSERT ... ON CONFLICT` atómico + advisory locks |
| 2 | **Duplicación en Passive Capture** | MEDIA | Check dentro de transacción + optimistic locking |
| 3 | **Manifest write race** | MEDIA | File locks + atomic rename |
| 4 | **No hay advisory locks para sync** | MEDIA | `flock()` advisory + mutex global |
| 5 | **Sesiones con colisiones** | MEDIA | UUID de sesión único por agente + lock |
| 6 | **Topic upsert no atómico** | BAJA | UPDATE atómico con versión vector |
| 7 | **JWT refresh race conditions** | MEDIA | Atomic token rotation |
| 8 | **No connection pooling** | BAJA | Connection pool con mutex |

### Arquitectura de Concurrencia

```
┌─────────────────────────────────────────────────────────────────┐
│                     AGENTE 1 (Claude Code)                      │
│  ┌─────────┐     ┌─────────────┐     ┌──────────────────────┐  │
│  │ Session │────▶│ Agent ID    │────▶│ UUID: agent-uuid     │  │
│  │ Manager │     │ Generator   │     │ + timestamp + random  │  │
│  └─────────┘     └─────────────┘     └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ Shared Lock Manager
┌─────────────────────────────────────────────────────────────────┐
│                    SHARED STATE (SQLite + Locks)                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ Write Lock   │  │ Read Lock    │  │ Advisory Lock        │  │
│  │ (Mutex)     │  │ (RWLock)     │  │ (flock per agent)    │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ WAL Mode - Multiple readers, ONE writer                    │   │
│  │ Busy timeout: 5000ms → fails fast on contention           │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     AGENTE 2 (OpenCode)                          │
│  ┌─────────┐     ┌─────────────┐     ┌──────────────────────┐  │
│  │ Session │────▶│ Agent ID    │────▶│ UUID: agent-uuid     │  │
│  │ Manager │     │ Generator   │     │ + timestamp + random  │  │
│  └─────────┘     └─────────────┘     └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Sesión Multi-Agente Única

```rust
struct UniqueSessionId {
    agent_id: String,      // "claude-code" / "opencode" / "cursor"
    agent_uuid: Uuid,      // Unique per agent instance
    hostname: String,      // Machine identifier
    timestamp: i64,        // Session start timestamp
    random_suffix: u32,   // Random to prevent collisions
}

impl UniqueSessionId {
    fn new(agent_type: &str) -> Self {
        Self {
            agent_id: agent_type.to_string(),
            agent_uuid: Uuid::new_v4(),
            hostname: hostname::get().unwrap_or_default(),
            timestamp: Timestamp::now().0,
            random_suffix: rand::random(),
        }
    }
    
    fn to_string(&self) -> String {
        format!("{}-{}-{}-{}-{:x}", 
            self.agent_id, self.agent_uuid, self.hostname, 
            self.timestamp, self.random_suffix)
    }
}
```

### Transacción Atómica con Deduplicación

```rust
impl Storage {
    fn add_observation_atomic(&self, obs: &Observation) -> Result<ObservationId> {
        // 1. Acquire write lock
        let _guard = self.write_lock.lock();
        
        // 2. Check for duplicate within SAME transaction (not separate query)
        let existing = self.check_duplicate_tx(&obs.content_hash)?;
        
        if let Some(id) = existing {
            // Update last_seen and increment duplicate_count
            self.increment_duplicate(id)?;
            return Ok(id);
        }
        
        // 3. INSERT with ON CONFLICT (no separate check-then-insert)
        let id = self.insert_with_conflict(obs)?;
        
        // 4. Commit atomically
        self.commit()?;
        
        Ok(id)
    }
    
    fn insert_with_conflict(&self, obs: &Observation) -> Result<ObservationId> {
        // SQLite UPSERT - atomic, no race window
        let sql = r#"
            INSERT INTO observations (sync_id, session_id, type, title, content, ...)
            VALUES (?, ?, ?, ?, ?, ...)
            ON CONFLICT(sync_id) DO UPDATE SET
                last_seen_at = CURRENT_TIMESTAMP,
                duplicate_count = duplicate_count + 1
            WHERE deleted_at IS NULL
        "#;
        // Execute atomically...
    }
}
```

### Advisory Locks para Sync

```rust
impl SyncManager {
    fn sync_with_lock(&self) -> Result<()> {
        // 1. Acquire advisory lock (works across processes)
        let lock_path = self.data_dir.join(".synapsis.lock");
        let fd = File::create(&lock_path)?;
        flock(fd.as_raw_fd(), LockFlags::LOCK_EX | LockFlags::LOCK_NB)?;
        
        // 2. Perform sync with exclusive access
        let result = self.do_sync();
        
        // 3. Release lock
        flock(fd.as_raw_fd(), LockFlags::LOCK_UN)?;
        
        result
    }
    
    fn write_manifest_atomic(&self, manifest: &Manifest) -> Result<()> {
        // Use rename for atomicity
        let temp_path = self.manifest_path.with_extension("tmp");
        let json = serde_json::to_vec(manifest)?;
        
        fs::write(&temp_path, json)?;
        fs::rename(&temp_path, &self.manifest_path)?; // Atomic on POSIX
        
        Ok(())
    }
}
```

---

## Arquitectura Hexagonal (Ports & Adapters)

```
┌─────────────────────────────────────────────────────────────────┐
│                        PRESENTATION LAYER                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────────┐│
│  │   MCP    │  │   HTTP   │  │   CLI    │  │      TUI       ││
│  │  Server  │  │   REST   │  │   Cmds   │  │   (BubbleTea)  ││
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └───────┬────────┘│
└───────┼────────────┼────────────┼─────────────────┼───────────┘
        │            │            │                 │
┌───────▼────────────▼────────────▼─────────────────▼───────────┐
│                        DOMAIN LAYER (Core)                      │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                    Memory Engine                        │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────┐  │  │
│  │  │ Session │ │Observa- │ │  Topic  │ │   Search    │  │  │
│  │  │ Manager │ │  tions  │ │  Keys   │ │   Engine    │  │  │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └──────┬──────┘  │  │
│  │       │          │           │             │          │  │
│  │       └──────────┴───────────┴─────────────┘          │  │
│  │              ┌─────────────────────┐                  │  │
│  │              │  Concurrency Manager │                  │  │
│  │              │  (Locks, Versioning) │                  │  │
│  │              └─────────────────────┘                  │  │
│  └────────────────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                   Security Layer                        │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │  │
│  │  │   PQC    │ │  Zero    │ │  Audit   │ │  Crypto  │  │  │
│  │  │ Crypto   │ │  Trust   │ │   Log    │ │  Utils  │  │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘  │  │
│  └────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
        │            │            │                 │
┌───────▼────────────▼────────────▼─────────────────▼───────────┐
│                    INFRASTRUCTURE LAYER                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────────┐│
│  │ SQLite + │  │  File    │  │  Sync    │  │    Network     ││
│  │  FTS5    │  │  Store   │  │  Engine  │  │   Transport    ││
│  │ (WAL+LCK)│  │ (Atomic) │  │(Advisory)│  │               ││
│  └──────────┘  └──────────┘  └──────────┘  └────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

---

## Soporte Multi-Plataforma (IDEs/CLIs)

### Matriz de Compatibilidad

| IDE/CLI | Estado | Protocolo | Notas |
|---------|--------|-----------|-------|
| **Claude Code** | ✅ | MCP + Plugin | Native support |
| **OpenCode** | ✅ | MCP | Configurado |
| **Cursor** | ✅ | MCP | Compatible |
| **Windsurf** | ✅ | MCP | Compatible |
| **VS Code (Copilot)** | ✅ | MCP | Requiere Copilot Edits |
| **Cody (Sourcegraph)** | ✅ | MCP | Compatible |
| **Gemini CLI** | ✅ | MCP | Compatible |
| **Codex (OpenAI)** | ✅ | MCP | Compatible |
| **Tabnine** | ⏳ | REST API | En desarrollo |
| **Amazon Q** | ⏳ | REST API | En desarrollo |

### Configuración por IDE

#### Claude Code
```json
// ~/.claudeirc
{
  "plugins": ["engram"],
  "mcpServers": {
    "synapsis": {
      "command": "synapsis",
      "args": ["mcp"]
    }
  }
}
```

#### OpenCode
```json
// ~/.config/opencode/opencode.json
{
  "mcp": {
    "synapsis": {
      "type": "local",
      "command": ["synapsis", "mcp"],
      "enabled": true
    }
  }
}
```

#### Cursor
```json
// ~/.cursor/mcp.json
{
  "mcpServers": {
    "synapsis": {
      "command": "synapsis",
      "args": ["mcp"]
    }
  }
}
```

### Plugin Templates

Located in `~/Projects/synapsis/plugins/`:

```
plugins/
├── claude-code/
│   ├── plugin.json
│   ├── hooks.json
│   └── scripts/
│       ├── session-start.sh
│       ├── session-stop.sh
│       └── post-compact.sh
├── opencode/
│   └── engram.ts
├── cursor/
│   └── mcp-config.json
├── windsurf/
│   └── mcp-config.json
└── vscode/
    └── package.json
```

---

## Estructura de Datos

### Observation (Multi-Agent Safe)
```rust
struct Observation {
    id: ObservationId,
    sync_id: SyncId,           // UUID único para dedup
    session_id: SessionId,      // Sesión única del agente
    agent_id: AgentId,          // ID del agente (nuevo)
    agent_instance: Uuid,       // Instancia única (nuevo)
    observation_type: ObservationType,
    title: String,
    content: String,
    tool_name: Option<String>,
    project: Option<String>,
    scope: Scope,
    topic_key: Option<String>,
    content_hash: ContentHash,
    version: u64,               // Optimistic locking (nuevo)
    revision_count: i32,
    duplicate_count: i32,
    last_seen_at: Option<Timestamp>,
    created_at: Timestamp,
    updated_at: Timestamp,
    deleted_at: Option<Timestamp>,
    // Seguridad
    integrity_hash: Option<String>,
    classification: Classification,
}
```

### Session (Multi-Agent Safe)
```rust
struct Session {
    id: SessionId,
    unique_session_id: String,  // UUID único multi-agente (nuevo)
    agent_id: AgentId,
    agent_instance: Uuid,
    hostname: String,
    project: String,
    directory: String,
    started_at: Timestamp,
    ended_at: Option<Timestamp>,
    summary: Option<String>,
    observation_count: i32,
}
```

---

## API Endpoints

### HTTP REST API (Puerto 7438)
```
GET  /health                 # Health check + agent status
POST /sessions               # Crear sesión única
POST /sessions/{id}/end      # Finalizar sesión
GET  /sessions/recent        # Sesiones recientes
GET  /sessions/active        # Agentes activos

POST /observations           # Añadir observación (atómica)
POST /observations/passive   # Passive capture
GET  /observations/recent   # Observaciones recientes
GET  /observations/{id}     # Obtener por ID
PATCH /observations/{id}     # Actualizar (optimistic lock)
DELETE /observations/{id}    # Eliminar

GET  /search?q=              # Búsqueda FTS5
GET  /timeline?obs_id=       # Timeline contextual
GET  /context                # Contexto formateado

POST /prompts                # Guardar prompt
GET  /prompts/recent         # Prompts recientes
GET  /prompts/search         # Buscar prompts

GET  /stats                  # Estadísticas
GET  /export                 # Exportar datos
POST /import                 # Importar datos

# Multi-agente endpoints
GET  /agents/active          # Agentes activos
GET  /agents/{id}/sessions   # Sesiones de un agente
GET  /lock/status            # Estado de locks

# Seguridad
GET  /audit/log              # Log de auditoría
GET  /security/status        # Estado de seguridad
POST /security/verify        # Verificar integridad
```

### MCP Tools
```
mem_save              # Guardar observación (atómica)
mem_search            # Buscar memoria
mem_context           # Contexto reciente
mem_session_summary   # Resumen de sesión
mem_session_start     # Iniciar sesión (genera UUID)
mem_session_end       # Finalizar sesión
mem_update            # Actualizar por ID (optimistic lock)
mem_delete            # Eliminar (soft/hard)
mem_get_observation   # Obtener contenido completo
mem_suggest_topic_key # Sugerir clave de tema
mem_capture_passive   # Extracción pasiva
mem_save_prompt       # Guardar prompt
mem_stats             # Estadísticas
mem_timeline          # Timeline contextual

# Nuevos tools Synapsis
mem_verify            # Verificar integridad
mem_classify          # Clasificar observación
mem_audit             # Obtener log de auditoría
mem_lock_status       # Estado de locks
```

---

## Issues de Engram Originales Corregidos

| Issue | Problema | Solución Synapsis |
|-------|----------|-------------------|
| Engram #175 | Feature request: backend persistente | Implementado por defecto |
| mem0 #3933 | update_memory tipo incorrecto | Type-safe desde el core |
| mem0 #3918 | JSON malformado por LLM | Validación estricta + fallback |
| mem0 #2762 | Entradas muy pequeñas | Chunking inteligente con límites |
| Claude Code #23769 | MEMORY.md corrupto | Transacciones ACID + WAL mode |
| Engram #25 | Session collision multi-agente | UUID único por agente |
| Engram race #1 | Duplicate observations concurrentes | INSERT ON CONFLICT atómico |
| Engram sync #1 | Manifest write race | File locks + atomic rename |

---

## Benchmarks Objetivo

| Métrica | Engram (Go) | Synapsis (Rust) |
|---------|-------------|-----------------|
| Tamaño binario | ~15MB | <5MB |
| Memoria RSS | ~50MB | <20MB |
| Latencia búsqueda | ~5ms | <1ms |
| Throughput writes | ~1000/s | >5000/s |
| Tiempo cold start | ~100ms | <20ms |
| Concurrencia writers | 1 | N (con locks) |
| Race conditions | Posibles | **0 garantizados** |

---

## Roadmap

### Fase 1: Core (Actual)
- [x] Diseño de arquitectura
- [x] Implementación Storage Engine (SQLite+FTS5)
- [x] Implementación Memory Engine
- [x] Capa PQC básica
- [x] **Soporte multi-agente** (locks, transacciones atómicas)
- [x] **Compatibilidad IDEs/CLIs**

### Fase 2: Seguridad Avanzada
- [ ] Implementación Kyber-1024 completa
- [ ] Implementación Dilithium-4 completa
- [ ] Zero-trust framework
- [ ] Audit logging inmutable

### Fase 3: Integración
- [ ] MCP Server completo
- [ ] HTTP API completa
- [ ] CLI commands
- [ ] Plugin templates para todos los IDEs

### Fase 4: Hardening
- [ ] Fuzzing tests
- [ ] Property-based tests
- [ ] Concurrency stress tests
- [ ] Security audit externo

---

## Autores

- methodwhite

## Licencia

MIT + Licencia de Seguridad PQC extendida disponible bajo request.
