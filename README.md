<p align="center">
  <img src="assets/logo.svg" alt="Synapsis" width="200" height="200">
</p>

<h1 align="center">Synapsis</h1>

<p align="center">
  <strong>Persistent Memory Engine for AI Agents</strong>
  <br>
  <sub>Built with Rust. MCP-native. Zero-trust security.</sub>
</p>

<p align="center">
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.95+-818CF8.svg" alt="Rust"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-BSL_1.1-6366F1.svg" alt="License"></a>
  <a href="https://github.com/MethodWhite/synapsis/releases"><img src="https://img.shields.io/github/v/release/MethodWhite/synapsis?color=A78BFA" alt="Release"></a>
</p>

Synapsis es un motor de memoria persistente multi-agente para asistentes IA, escrito en **Rust puro** con SQLite + FTS5 y criptografía post-cuántica opcional. Implementa el [Model Context Protocol (MCP)](https://modelcontextprotocol.io) para dar a los agentes LLM memoria duradera y buscable a través de sesiones.

> **Zero runtime dependencies.** Synapsis está escrito completamente en Rust — no requiere Python, Node.js, JVM, ni ningún intérprete. Docker y Kubernetes son opciones de despliegue opcionales; el binario nativo corre standalone en cualquier plataforma.

---

## Quick Start

```bash
git clone https://github.com/methodwhite/synapsis.git
cd synapsis
cargo build --release

# MCP server (stdio) — para agentes IA locales
./target/release/synapsis-mcp

# Auto-configurar MCP para herramientas detectadas
./target/release/synapsis-autoconfig --apply
```

**Prerrequisitos:** Rust 1.95+ ([install](https://rustup.rs))

---

## Binarios

| Binario | Propósito |
|---------|-----------|
| `synapsis` | Servidor HTTP/SSE MCP multi-agente |
| `synapsis-mcp` | Servidor MCP stdio (single-agent) |
| `synapsis-server` | Servidor HTTP + QUIC |
| `synapsis-autoconfig` | Auto-detección y configuración MCP multiplataforma |
| `synapsis-ollama` | Integración con Ollama |

---

## Características

- **Memoria Persistente** — Guarda, busca y recupera observaciones con FTS5 + BM25 ranking
- **Multi-agente** — Base de datos compartida con locks distribuidos, colas de tareas, y orquestación
- **MCP-native** — Compatible con cualquier cliente MCP (OpenCode, Claude Code, Cursor, Gemini CLI, etc.)
- **Auto-Config MCP** — Detecta herramientas instaladas y genera configs MCP automáticamente
- **Puente de Sesiones** — Comparte sesiones y observaciones entre todas las plataformas conectadas
- **Catálogo de Plataformas** — 30+ plataformas registradas (occidentales + chinas)
- **Descubrimiento de Red** — mDNS para encontrar nodos y servidores MCP en la red local
- **Seguridad** — Cifrado AES-256-GCM, Kyber-768 (PQC), HMAC, TPM 2.0, TOTP
- **Watchdog** — Monitoreo de integridad del sistema de archivos
- **Anti-Brick** — Protección contra comandos destructivos
- **Recicle Bin** — Categorización con TTL, búsqueda y undelete
- **Multiplataforma** — Linux, macOS, Windows

---

## MCP Tools

### Memoria
| Tool | Descripción |
|------|-------------|
| `mem_save` | Guardar observación en memoria persistente |
| `mem_search` | Buscar en memoria a través de sesiones |
| `mem_context` | Contexto reciente de sesiones |
| `mem_timeline` | Línea de tiempo cronológica |
| `mem_stats` | Estadísticas de memoria |
| `mem_get_observation` | Obtener observación por ID |
| `mem_update` | Actualizar observación existente |
| `mem_delete` | Eliminar observación (soft-delete) |

### Sesiones
| Tool | Descripción |
|------|-------------|
| `mem_session_start` | Iniciar sesión de trabajo |
| `mem_session_end` | Finalizar sesión con resumen |
| `mem_session_summary` | Resumen de una sesión |
| `shared_sessions_list` | Listar sesiones activas entre plataformas |
| `shared_sessions_by_project` | Sesiones activas por proyecto |
| `shared_sessions_broadcast` | Compartir observación entre sesiones |

### Agentes y Tareas
| Tool | Descripción |
|------|-------------|
| `agent_register` | Registrar un nuevo agente |
| `agent_list` | Listar agentes registrados |
| `agent_list_by_project` | Agentes por proyecto |
| `agent_unregister` | Eliminar un agente |
| `task_create` | Crear tarea |
| `task_list` | Listar tareas |
| `skill_register` | Registrar habilidad |
| `skill_list` | Listar habilidades |

### Seguridad
| Tool | Descripción |
|------|-------------|
| `pqc_encrypt` | Cifrar datos con criptografía post-cuántica |
| `antibrick_scan` | Escanear comando por amenazas destructivas |
| `antibrick_enable` | Activar/desactivar protección anti-brick |
| `antibrick_stats` | Estadísticas de protección |
| `auth_tpm_status` | Estado de TPM |
| `auth_check_permission` | Verificar permisos |
| `auth_classify_agent` | Clasificar tipo de agente |

### Watchdog
| Tool | Descripción |
|------|-------------|
| `watchdog_stats` | Estadísticas del watchdog |
| `watchdog_verify` | Verificar integridad de archivos |
| `watchdog_snapshot` | Crear snapshot de integridad |
| `watchdog_check_path` | Verificar si un path está protegido |
| `watchdog_events` | Eventos recientes del watchdog |

### Descubrimiento
| Tool | Descripción |
|------|-------------|
| `discovery_scan` | Escanear el sistema en busca de herramientas y servidores MCP |
| `ghost_audit` | Auditoría proactiva de archivos |

### Base de Datos
| Tool | Descripción |
|------|-------------|
| `db_backup` | Backup de la base de datos |
| `db_integrity` | Verificar integridad de la DB |
| `db_prune` | Eliminar observaciones antiguas |
| `db_vacuum` | Recuperar espacio en la DB |
| `db_health` | Estado de salud de la DB |

### Red
| Tool | Descripción |
|------|-------------|
| `mcp_call` | Llamar a otro servidor MCP vía HTTP |
| `browser_navigate` | Navegar a URL como cliente HTTP |
| `browser_snapshot` | Obtener snapshot estructurado de una página |

### Juicio y Relaciones
| Tool | Descripción |
|------|-------------|
| `mem_judge` | Registrar juicio de conflicto entre memorias |
| `mem_compare` | Comparación semántica entre observaciones |
| `mem_merge_projects` | Fusionar observaciones entre proyectos |

---

## Plataformas Soportadas

### Auto-detección y Configuración Automática

Synapsis detecta automáticamente qué herramientas están instaladas y genera los archivos MCP correspondientes:

**CLIs:** OpenCode, Claude Code, Gemini CLI, Cline, aider, fabric, shell_gpt, Codex CLI, AutoGPT, gpt-engineer, sweep

**IDEs:** VS Code + Copilot, Cursor, Windsurf, JetBrains (IntelliJ, PyCharm, GoLand, WebStorm), Android Studio, Continue.dev, Cody, Tabnine, Amazon Q Developer, Cline (extensión), Roo Code

**Plataformas Chinas:** DeepSeek, 月之暗面 Kimi, 智谱 GLM/ChatGLM, 阿里 Qwen/通义, 百度 ERNIE/文心, 字节跳动 豆包, 阶跃星辰 Step, MiniMax, 讯飞 星火, 百川 Baichuan

```bash
# Detectar y configurar
synapsis-autoconfig --apply

# Ver qué se detectaría (dry-run)
synapsis-autoconfig

# Monitoreo continuo
synapsis-autoconfig --apply --watch
```

---

## Compartir Sesiones Entre Plataformas

El Session Bridge permite que observaciones guardadas desde una herramienta sean visibles desde todas las demás:

```bash
# Listar sesiones activas
scripts/session-share.sh list

# Compartir observación entre sesiones del mismo proyecto
scripts/session-share.sh broadcast "hallazgo importante sobre la API"

# Daemon de auto-enlace
scripts/session-autolink.sh --daemon
```

---

## Casos de Uso

Ocho flujos de trabajo reales que muestran Synapsis en acción:

| # | Caso de Uso | Descripción |
|---|-------------|-------------|
| 1 | [Cross-Platform Memory](docs/USE_CASES.md#use-case-1-cross-platform-memory) | Guardar desde OpenCode, recuperar desde Cursor |
| 2 | [Session Continuity](docs/USE_CASES.md#use-case-2-session-continuity) | Empezar en Gemini CLI, continuar en VS Code |
| 3 | [Auto-Discovery + Auto-Config](docs/USE_CASES.md#use-case-3-auto-discovery--auto-config) | Plug and play: nueva herramienta detectada automáticamente |
| 4 | [Secure Production Deployment](docs/USE_CASES.md#use-case-4-secure-production-deployment) | HTTPS + API keys para uso en equipo |
| 5 | [MCP Discovery Network](docs/USE_CASES.md#use-case-5-mcp-discovery-network) | Encontrar y conectar nodos Synapsis remotos |
| 6 | [Chinese Platform Integration](docs/USE_CASES.md#use-case-6-chinese-platform-integration) | Usar Synapsis con DeepSeek, Qwen, Kimi |
| 7 | [CI/CD Pipeline Memory](docs/USE_CASES.md#use-case-7-cicd-pipeline-memory) | Synapsis en GitHub Actions |
| 8 | [Backup and Disaster Recovery](docs/USE_CASES.md#use-case-8-backup-and-disaster-recovery) | Backup cifrado con verificación de integridad |

Cada caso incluye comandos paso a paso, arquitectura técnica y output esperado.
Ver [docs/USE_CASES.md](docs/USE_CASES.md) para la guía completa.

---

## Arquitectura

```
┌─────────────────────────────────────────────┐
│              PRESENTATION LAYER              │
│  MCP stdio  HTTP/SSE  CLI  TUI (ratatui)    │
├─────────────────────────────────────────────┤
│               DOMAIN LAYER                   │
│  Memory Engine  Session Manager  Security    │
│  Auth  Orchestrator  Workers  Task Queue    │
│  Watchdog  Anti-Brick  Recycle Bin  Vault    │
├─────────────────────────────────────────────┤
│            INFRASTRUCTURE LAYER               │
│  SQLite+FTS5  File Store  Agent Registry    │
│  Context Mgmt  Discovery  Session Bridge    │
│  Platform Catalog  MCP Auto-Config           │
└─────────────────────────────────────────────┘
```

---

## Almacenamiento

Synapsis usa **SQLite** con **FTS5** para búsqueda de texto completo:

- **Observations** — Título, contenido, proyecto, tipo, ámbito, hash de integridad
- **Sessions** — Ciclo de vida de sesiones multi-agente
- **Task Queue** — Coordinación de tareas con prioridad y reintentos
- **Agent Registry** — Habilidades, capacidad y descubrimiento
- **Context Registry** — Estados hot/warm/cold con compresión
- **Locks** — Mutex distribuido para acceso concurrente

---

## Variables de Entorno

| Variable | Descripción |
|----------|-------------|
| `SYNAPSIS_DB_KEY` | Clave de cifrado hex-encoded (SQLCipher) |
| `SYNAPSIS_DB_KEY_BASE64` | Clave de cifrado Base64 |
| `SYNAPSIS_DATA_DIR` | Directorio de datos personalizado |
| `SYNAPSIS_QUIET` | Suprimir output informativo |
| `SYNAPSIS_LOG` | Nivel de log (debug, info, warn, error) |
| `SYNAPSIS_API_KEYS` | API keys separadas por coma para auth |
| `SYNAPSIS_PORT` | Puerto HTTP (default: 7438) |
| `SYNAPSIS_URL` | URL del servidor MCP (para scripts) |
| `SYNAPSIS_AUTH` | Habilitar clasificador de agentes |

---

## Documentación

| Documento | Descripción |
|-----------|-------------|
| [PROJECT.md](PROJECT.md) | Identidad y stack tecnológico |
| [SPEC.md](SPEC.md) | Especificación técnica detallada |
| [Arquitectura](assets/architecture.drawio) | Diagrama de arquitectura (draw.io) |
| [Multi-Agente](docs/MULTI-AGENT.md) | Orquestación multi-agente |
| [Despliegue](docs/DEPLOYMENT_GUIDE.md) | Guía de despliegue en producción |
| [Roadmap](docs/ROADMAP.md) | Hoja de ruta de desarrollo |

---

## Licencia

Business Source License 1.1 — ver [LICENSE](LICENSE) para detalles.

Uso comercial requiere licencia separada para entidades con 3+ empleados o más de $100k en ingresos anuales. Uso no comercial, educativo y personal es gratuito. Cambio a Apache 2.0 el 23 de marzo de 2030.

---

<p align="center">
  <sub>Built with Rust by MethodWhite</sub>
</p>

- [JMT x402 Agent Tools](https://jmt-x402-proxy.jmthomasofficial.workers.dev) — 25 paid x402 endpoints on Base mainnet: web search, AI analysis, crypto/stock data, SEC filings, company intel, news, sentiment, macro dashboard. $0.001-$0.15/call USDC. Local LLM-powered.