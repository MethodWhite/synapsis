# Synapsis TCP Server API Documentation

## Conexión

```
Host: 127.0.0.1
Port: 7438
Protocol: JSON-RPC 2.0 over TCP
```

## Formato de Mensaje

```json
{
  "jsonrpc": "2.0",
  "method": "<nombre_del_metodo>",
  "params": {
    "arguments": {
      "<arg1>": "<valor1>",
      "<arg2>": "<valor2>"
    }
  },
  "id": <numero_opcional>
}
```

## Métodos Disponibles

### Session Management

#### `session_register`
Registrar nueva sesión de agente.

```json
{
  "method": "session_register",
  "params": {
    "arguments": {
      "agent_type": "opencode|cursor|claude|jetbrains",
      "project": "mi-proyecto"
    }
  },
  "id": 1
}
```

**Response:**
```json
{
  "id": 1,
  "jsonrpc": "2.0",
  "result": {
    "session_id": "opencode-abc123-1774153000",
    "auto_reconnect": true,
    "reconnected": false
  }
}
```

#### `session_reconnect`
Reconectar sesión existente.

```json
{
  "method": "session_reconnect",
  "params": {
    "arguments": {
      "agent_type": "opencode",
      "project": "mi-proyecto"
    }
  },
  "id": 2
}
```

#### `agent_heartbeat`
Enviar heartbeat para mantener sesión activa.

```json
{
  "method": "agent_heartbeat",
  "params": {
    "arguments": {
      "session_id": "opencode-abc123-1774153000",
      "task": "build|search|edit|etc"
    }
  },
  "id": 3
}
```

### Agent Coordination

#### `agents_active`
Listar agentes activos.

```json
{
  "method": "agents_active",
  "params": {},
  "id": 4
}
```

**Response:**
```json
{
  "result": {
    "agents": [
      {
        "session_id": "opencode-abc123",
        "agent_type": "opencode",
        "project": "mi-proyecto",
        "current_task": "build",
        "last_heartbeat": 1774153000
      }
    ]
  }
}
```

### Distributed Locks

#### `lock_acquire`
Adquirir lock distribuido.

```json
{
  "method": "lock_acquire",
  "params": {
    "arguments": {
      "session_id": "opencode-abc123",
      "lock_key": "resource-name",
      "ttl": 300
    }
  },
  "id": 5
}
```

**Response:**
```json
{
  "result": {
    "acquired": true
  }
}
```

#### `lock_release`
Liberar lock.

```json
{
  "method": "lock_release",
  "params": {
    "arguments": {
      "lock_key": "resource-name"
    }
  },
  "id": 6
}
```

### Task Queue

#### `task_create`
Crear tarea en cola.

```json
{
  "method": "task_create",
  "params": {
    "arguments": {
      "project": "mi-proyecto",
      "task_type": "build|test|deploy|search",
      "payload": "datos de la tarea",
      "priority": 10
    }
  },
  "id": 7
}
```

#### `task_claim`
Reclamar tarea pendiente.

```json
{
  "method": "task_claim",
  "params": {
    "arguments": {
      "session_id": "opencode-abc123",
      "task_type": "build"
    }
  },
  "id": 8
}
```

### Context Management

#### `context_export`
Exportar contexto del proyecto.

```json
{
  "method": "context_export",
  "params": {
    "arguments": {
      "project": "mi-proyecto"
    }
  },
  "id": 9
}
```

#### `context_import`
Importar contexto.

```json
{
  "method": "context_import",
  "params": {
    "arguments": {
      "context": "{\"project\":\"mi-proyecto\",\"chunks\":[...]}"
    }
  },
  "id": 10
}
```

### Memory Operations

#### `memory_search_fts`
Búsqueda full-text con FTS5.

```json
{
  "method": "memory_search_fts",
  "params": {
    "arguments": {
      "query": "authentication bug",
      "project": "mi-proyecto",
      "limit": 20
    }
  },
  "id": 11
}
```

#### `stats`
Obtener estadísticas.

```json
{
  "method": "stats",
  "params": {},
  "id": 12
}
```

## Ejemplos de Uso

### Bash/Netcat

```bash
# Registrar sesión
echo '{"jsonrpc":"2.0","method":"session_register","params":{"arguments":{"agent_type":"my-agent","project":"test"}},"id":1}' | nc 127.0.0.1 7438

# Enviar heartbeat
echo '{"jsonrpc":"2.0","method":"agent_heartbeat","params":{"arguments":{"session_id":"my-agent-abc-123","task":"build"}},"id":2}' | nc 127.0.0.1 7438

# Buscar en memoria
echo '{"jsonrpc":"2.0","method":"memory_search_fts","params":{"arguments":{"query":"authentication","limit":5}},"id":3}' | nc 127.0.0.1 7438
```

### Python

```python
import socket
import json

def send_request(method, args={}):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.connect(('127.0.0.1', 7438))
    
    request = {
        "jsonrpc": "2.0",
        "method": method,
        "params": {"arguments": args},
        "id": 1
    }
    
    sock.send(json.dumps(request).encode() + b'\n')
    response = sock.recv(4096).decode()
    sock.close()
    
    return json.loads(response)

# Ejemplo
result = send_request('session_register', {
    'agent_type': 'python-agent',
    'project': 'my-project'
})
print(result)
```

### Node.js

```javascript
const net = require('net');

function sendRequest(method, args = {}) {
    return new Promise((resolve, reject) => {
        const client = net.createConnection({ port: 7438, host: '127.0.0.1' }, () => {
            const request = {
                jsonrpc: '2.0',
                method: method,
                params: { arguments: args },
                id: 1
            };
            client.write(JSON.stringify(request) + '\n');
        });
        
        client.on('data', (data) => {
            resolve(JSON.parse(data.toString()));
            client.end();
        });
        
        client.on('error', reject);
    });
}

// Ejemplo
sendRequest('agents_active').then(console.log);
```

## Códigos de Error

| Código | Descripción |
|--------|-------------|
| -32600 | Invalid Request |
| -32601 | Method Not Found |
| -32602 | Invalid Params |
| -32000 | Server Error |

## Timeout y Heartbeats

- **Heartbeat TTL:** 120 segundos
- **Lock TTL:** 90-300 segundos (configurable)
- **Recomendación:** Enviar heartbeat cada 25-30 segundos

## Multi-Agent Coordination

1. Cada agente registra su sesión
2. Adquiere lock antes de operación crítica
3. Envía heartbeats periódicos
4. Reclama tareas de la cola
5. Libera locks al completar
