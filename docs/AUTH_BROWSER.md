# Synapsis - Authenticated Browser Module

## Nuevas capacidades agregadas (2026-04-13)

### Herramientas MCP nuevas

| Herramienta | Descripción |
|-------------|-------------|
| `auth_navigate` | Navega a una URL con soporte de autenticación (login, cookies, sesión persistente) |
| `auth_extract` | Extrae contenido de una sesión autenticada usando selectores CSS |
| `auth_navigate_session` | Navega a una nueva URL dentro de una sesión autenticada existente |
| `auth_clear_session` | Limpia/elimina una sesión de navegador guardada |
| `auth_list_sessions` | Lista todas las sesiones autenticadas guardadas |

### Cómo funciona

1. **Sesión persistente**: Las cookies se guardan en `~/.synapsis/browser_sessions/<session_id>.json`
2. **Auto-login**: Si proporcionas credenciales y selectores, hace login automáticamente
3. **Reutilización**: Sesiones se restauran en llamadas posteriores sin re-login
4. **Headless Chrome**: Usa `headless_chrome` crate con Chromium invisible

### Ejemplo de uso via MCP

```json
// 1. Login + navegar a página protegida
{
  "method": "tools/call",
  "params": {
    "name": "auth_navigate",
    "arguments": {
      "url": "https://netacad.com/courses/cyberops",
      "session_id": "netacad-cyberops",
      "login_url": "https://netacad.com/login",
      "login_selector_user": "#username",
      "login_selector_pass": "#password",
      "username": "tu@email.com",
      "password": "tu_password",
      "login_button_selector": "button[type='submit']"
    }
  }
}

// 2. Extraer contenido del curso
{
  "method": "tools/call",
  "params": {
    "name": "auth_extract",
    "arguments": {
      "session_id": "netacad-cyberops",
      "selector": ".course-content, .chapter-text, article"
    }
  }
}

// 3. Navegar a otro capítulo
{
  "method": "tools/call",
  "params": {
    "name": "auth_navigate_session",
    "arguments": {
      "session_id": "netacad-cyberops",
      "url": "https://netacad.com/courses/cyberops/chapter-2"
    }
  }
}

// 4. Limpiar sesión cuando termines
{
  "method": "tools/call",
  "params": {
    "name": "auth_clear_session",
    "arguments": {
      "session_id": "netacad-cyberops"
    }
  }
}
```

### Arquitectura

```
MCP Client (Qwen/Opencode)
    │
    ├── auth_navigate → Synapsis MCP Server
    │                      │
    │                      └── headless_chrome → Chromium (headless)
    │                           │
    │                           ├── Login flow (selectores CSS + JS)
    │                           ├── Cookie persistence (JSON)
    │                           └── Content extraction (JS eval)
    │
    └── auth_extract → Reuse saved session cookies
```

### Seguridad

- 🔒 Cookies almacenadas en `~/.synapsis/browser_sessions/`
- 🔒 Sesiones identificadas por ID (no por URL)
- 🔒 Clear session para borrar credenciales cacheadas
- ⚠️ **No almacenamos contraseñas** — se pasan en cada llamada

### Compilación

```bash
cargo build --release --features browser
```

### Archivos modificados

- `src/tools/auth_browser.rs` — Nuevo módulo (516 líneas)
- `src/tools/mod.rs` — Registro del módulo
- `src/presentation/mcp/server.rs` — Import, schema, dispatch, actions
- `Cargo.toml` — Ya tenía `headless_chrome` como dependencia opcional

---

**Versión**: Synapsis v0.1.0 + Auth Browser Module
**Fecha**: 2026-04-13
