# Synapsis MCP Wrapper for Gemini CLI

Wrapper de integración entre Gemini CLI y Synapsis MCP server con soporte PQC.

## Instalación

```bash
# Copiar a PATH
cp synapsis-mcp.sh /usr/local/bin/synapsis-mcp
chmod +x /usr/local/bin/synapsis-mcp

# O usar desde el directorio
export PATH="$PATH:/path/to/synapsis/plugins/gemini-cli"
```

## Configuración

```bash
# Variables de entorno
export SYNAPSIS_SERVER="127.0.0.1:7438"
export SYNAPSIS_PROJECT="mi-proyecto"

# Registrar sesión (automático al primer uso)
synapsis-mcp register

# Verificar sesión
cat /tmp/synapsis-gemini-session
```

## Comandos Disponibles

| Comando | Descripción | Ejemplo |
|---------|-------------|---------|
| `register` | Registrar nueva sesión | `synapsis-mcp register` |
| `heartbeat` | Enviar heartbeat | `synapsis-mcp heartbeat "coding"` |
| `save` | Guardar contexto | `synapsis-mcp save "Bug Fix" "Fixed auth bypass"` |
| `search` | Buscar en memoria | `synapsis-mcp search "authentication"` |
| `context` | Obtener contexto global | `synapsis-mcp context` |
| `lock-acquire` | Adquirir lock | `synapsis-mcp lock-acquire build 300` |
| `lock-release` | Liberar lock | `synapsis-mcp lock-release build` |
| `claim` | Reclamar tarea | `synapsis-mcp claim build` |

## Integración con Gemini CLI

### Hook pre-command

```bash
# ~/.gemini/hooks/pre-command.sh
#!/bin/bash
synapsis-mcp heartbeat "gemini-command: $1"
```

### Hook post-command

```bash
# ~/.gemini/hooks/post-command.sh
#!/bin/bash
if [ -n "$COMMAND_OUTPUT" ]; then
    synapsis-mcp save "Command Output" "$COMMAND_OUTPUT"
fi
```

## Ejemplos de Uso

### Búsqueda de contexto antes de coding

```bash
# Buscar bugs similares
synapsis-mcp search "authentication bypass"

# Ver reglas del proyecto
synapsis-mcp context
```

### Coordinación multi-agente

```bash
# Adquirir lock antes de build
synapsis-mcp lock-acquire synapsis-build 300

# Reclamar tarea de build
synapsis-mcp claim build

# Liberar lock después de completar
synapsis-mcp lock-release synapsis-build
```

### Guardar contexto de sesión

```bash
# Guardar decisión de arquitectura
synapsis-mcp save "Architecture Decision" "Using SQLite with FTS5 for memory storage"

# Guardar bug fix
synapsis-mcp save "Bug Fix" "Fixed race condition in agent heartbeat"
```

## PQC Security

El wrapper soporta cifrado post-cuántico cuando Synapsis está compilado con `--features pqc`:

```bash
# Verificar soporte PQC
synapsis-mcp pqc-status

# Habilitar cifrado PQC para contexto sensible
synapsis-mcp save --pqc "Security Credentials" "encrypted-data"
```

## Troubleshooting

### Error: Server not responding
```bash
# Verificar servidor
nc -zv 127.0.0.1 7438

# Reiniciar sesión
rm /tmp/synapsis-gemini-session
synapsis-mcp register
```

### Error: Session expired
```bash
# Re-registrar
synapsis-mcp register
```
