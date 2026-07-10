# Synapsis

**Idioma:** Rust (edition 2024)
**Propósito:** Motor de memoria persistente multi-agente con criptografía post-cuántica
**Protocolo:** MCP (Model Context Protocol) + HTTP REST + JSON-RPC
**Transporte:** stdio (MCP), HTTP (REST API), QUIC (streaming)
**Base de datos:** SQLite + FTS5 (WAL mode)
**Seguridad:** AES-256-GCM, Kyber-768 (ML-KEM), HMAC-SHA256, TOTP, TPM 2.0

## Stack tecnológico

| Capa | Tecnología |
|------|-----------|
| Lenguaje | Rust (MSRV 1.95) |
| Framework MCP | Propietario (JSON-RPC sobre stdio/HTTP) |
| CLI | synapsis, synapsis-mcp, synapsis-server, synapsis-autoconfig |
| UI | ratatui (TUI, opcional) |
| Scripts | **Bash exclusivamente** — sin Python, sin Node.js |
| Base de datos | SQLite v3.48+ con FTS5 |
| Cifrado | aes-gcm, hmac, sha2, sha1 (TOTP), pqcrypto (Kyber-768) |
| Red | quinn (QUIC), mdns-sd (descubrimiento local) |

## Binarios

| Binario | Propósito |
|---------|-----------|
| `synapsis` | Servidor HTTP/SSE MCP + CLI |
| `synapsis-mcp` | Servidor MCP puro (stdio) |
| `synapsis-server` | Servidor HTTP + QUIC |
| `synapsis-autoconfig` | Auto-configuración MCP multiplataforma |
| `synapsis-ollama` | Integración con Ollama |

## Scripts

Todos los scripts en `scripts/` son **Bash** — cero dependencias de Python, Node.js o Ruby.

| Script | Propósito |
|--------|-----------|
| `autoconfig.sh` | Detecta herramientas instaladas y genera configs MCP |
| `install-mcp.sh` | Instalación rápida de Synapsis MCP |
| `session-share.sh` | CLI para compartir sesiones entre plataformas |
| `session-autolink.sh` | Daemon de auto-detección de sesiones activas |

## No dependencias externas

Synapsis **no requiere** Python, Node.js, Ruby, ni ningún runtime externo.
Todo está compilado en Rust como binarios estáticos.
