# Synapsis Multi-Agent Workflow

## Arquitectura de Coordinación

```
┌─────────────┐         ┌─────────────┐
│  qwen-code  │◄───────►│  opencode   │
│  (active)   │  TCP    │  (running)  │
└──────┬──────┘  :7438  └──────┬──────┘
       │                       │
       └───────────┬───────────┘
                   │
       ┌───────────▼───────────┐
       │   Synapsis Server     │
       │   - Task Queue        │
       │   - Distributed Locks │
       │   - Shared Context    │
       └───────────────────────┘
```

## Flujo de Coordinación

### 1. Registro de Agente

```bash
# Cada agente se registra al iniciar
echo '{"method":"session_register","params":{"arguments":{"agent_type":"opencode","project":"synapsis"}}}' | nc 127.0.0.1 7438
```

### 2. Heartbeat Periódico

```bash
# Enviar heartbeat cada 25-30 segundos
echo '{"method":"agent_heartbeat","params":{"arguments":{"session_id":"SESSION_ID","task":"current-task"}}}' | nc 127.0.0.1 7438
```

### 3. Adquisición de Lock para Builds

```bash
# Antes de build crítica, adquirir lock (300s TTL)
echo '{"method":"lock_acquire","params":{"arguments":{"session_id":"SESSION_ID","lock_key":"synapsis-build","ttl":300}}}' | nc 127.0.0.1 7438
```

### 4. Claim de Tareas

```bash
# Reclamar tarea de la cola
echo '{"method":"task_claim","params":{"arguments":{"session_id":"SESSION_ID","task_type":"build"}}}' | nc 127.0.0.1 7438
```

### 5. Compartir Contexto

```bash
# Guardar contexto en chunk compartido
echo '{"method":"chunk_create","params":{"arguments":{"project":"synapsis","title":"Build Status","content":"Build completed successfully"}}}' | nc 127.0.0.1 7438

# Leer contexto de otros agentes
echo '{"method":"chunk_get","params":{"arguments":{"project":"synapsis"}}}' | nc 127.0.0.1 7438
```

## Estados del Agente

| Estado | Descripción | Acción |
|--------|-------------|--------|
| `active` | Heartbeat < 30s | Trabajando normal |
| `idle` | Heartbeat > 30s | Verificar si está bloqueado |
| `inactive` | Heartbeat > 120s | Marcar como inactivo |

## Resolución de Conflictos

### Lock Expirado
- Si lock > TTL, otro agente puede adquirir
- Usar `lock_release` explícito al completar

### Tarea Reclamada por Múltiples
- Primero en claim gana
- Verificar `task_claim` response

### Contexto Desactualizado
- Usar timestamps en chunks
- Verificar `updated_at` antes de usar

## Ejemplo: Build Coordinada

```bash
# Agente 1: Adquiere lock
lock = lock_acquire("build-lock", ttl=300)

# Agente 1: Inicia build
task = task_claim("build")
run_build()

# Agente 2: Espera o trabaja en otra tarea
if lock_acquired_by_other():
    task = task_claim("integration")

# Agente 1: Completa y libera
task_complete(task, result="success")
lock_release("build-lock")
```

## Mejores Prácticas

1. **Heartbeat frecuente** - Cada 25s máximo
2. **Locks con TTL apropiado** - 300s para builds largas
3. **Contexto descriptivo** - Incluir timestamps y agente
4. **Liberar recursos** - Siempre release locks
5. **Verificar estado** - Check agents_active antes de coordinar
