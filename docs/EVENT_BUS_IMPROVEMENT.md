# Event Bus Improvement - Push Notifications

## Problema Actual

El sistema actual usa polling para verificar eventos:
- Los agentes consultan periódicamente `agents_active`
- Las tareas requieren `task_claim` manual
- No hay notificación en tiempo real de cambios

## Solución: Push Notifications

### Arquitectura Propuesta

```
┌─────────────┐         ┌─────────────┐
│   Agentes   │◄────────│  Event Bus  │
│  (clients)  │  WebSocket│  Server   │
└─────────────┘         └─────────────┘
                              │
                              ▼
                       ┌─────────────┐
                       │   SQLite    │
                       │   FTS5      │
                       └─────────────┘
```

### Eventos a Notificar

| Evento | Trigger | Payload |
|--------|---------|---------|
| `task.created` | Nueva tarea en cola | `{task_id, type, priority}` |
| `task.completed` | Tarea completada | `{task_id, result}` |
| `agent.joined` | Agente se registra | `{agent_id, project}` |
| `agent.left` | Agente inactivo | `{agent_id, reason}` |
| `context.updated` | Chunk actualizado | `{project, chunk_id}` |
| `lock.acquired` | Lock adquirido | `{lock_key, agent_id}` |
| `lock.released` | Lock liberado | `{lock_key, agent_id}` |

### Implementación en Rust

```rust
// WebSocket server para push notifications
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use futures_util::{SinkExt, StreamExt};

pub struct EventBus {
    clients: DashMap<SessionId, WebSocketSender>,
    db: Arc<Database>,
}

impl EventBus {
    pub async fn start(&self, port: u16) {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).await.unwrap();
        
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(handle_connection(stream));
        }
    }
    
    pub fn publish(&self, event: &str, payload: &Value) {
        // Enviar a todos los clientes suscritos
        for client in self.clients.iter() {
            let msg = json!({
                "type": "event",
                "event": event,
                "payload": payload,
                "timestamp": Timestamp::now().0
            });
            let _ = client.send(msg.to_string());
        }
    }
}
```

### Cliente JavaScript (Opencode)

```javascript
const ws = new WebSocket('ws://127.0.0.1:8080/events');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    switch(data.event) {
        case 'task.created':
            console.log('Nueva tarea:', data.payload);
            // Auto-claim si es de mi tipo
            break;
        case 'agent.joined':
            console.log('Agente activo:', data.payload);
            break;
        case 'context.updated':
            console.log('Contexto actualizado:', data.payload);
            // Refrescar caché local
            break;
    }
};
```

### Beneficios

1. **Menor latencia** - Notificación instantánea vs polling cada 30s
2. **Menor carga** - Sin consultas periódicas innecesarias
3. **Mejor UX** - Los agentes reaccionan inmediatamente
4. **Escalabilidad** - WebSocket maneja miles de conexiones

### Migración Gradual

1. Mantener polling como fallback
2. Agentes nuevos usan WebSocket
3. Deprecar polling después de 30 días

### Configuración

```json
{
  "event_bus": {
    "enabled": true,
    "websocket_port": 8080,
    "reconnect_interval": 5000,
    "events": ["task.*", "agent.*", "context.*", "lock.*"]
  }
}
```
