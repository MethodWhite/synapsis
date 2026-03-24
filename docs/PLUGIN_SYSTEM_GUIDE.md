# 🧩 Synapsis Plugin System - Guía Completa

## Overview

El sistema de plugins de Synapsis permite extender las capacidades del core mediante plugins cargados dinámicamente (.so/.dylib/.dll).

## Arquitectura

```
┌─────────────────────────────────────────────────────────┐
│              Synapsis Application                        │
│  ┌───────────────────────────────────────────────────┐  │
│  │          Plugin Registry                           │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌───────────┐ │  │
│  │  │ Static      │  │ Dynamic     │  │ Dynamic   │ │  │
│  │  │ Plugins     │  │ Plugin .so  │  │ Plugin    │ │  │
│  │  │ (compiled)  │  │ (loaded)    │  │ (.dylib)  │ │  │
│  │  └─────────────┘  └─────────────┘  └───────────┘ │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Extension Points

Los plugins pueden registrarse en los siguientes puntos de extensión:

| Extension Point | Descripción | Ejemplo de Uso |
|----------------|-------------|----------------|
| `CryptoProvider` | Proveedores de criptografía | PQC, HSM, TPM |
| `AuthProvider` | Autenticación y autorización | JWT, OAuth, LDAP |
| `StorageBackend` | Backends de almacenamiento | S3, IPFS, Database |
| `LlmProvider` | Proveedores de LLM | Ollama, OpenAI, Local |
| `WorkerAgent` | Agentes trabajadores | Code, Search, Shell |
| `RpcHandler` | Manejadores RPC personalizados | API endpoints |
| `TaskQueue` | Implementaciones de colas | Redis, RabbitMQ |
| `DatabaseAdapter` | Adaptadores de base de datos | PostgreSQL, MySQL |
| `Monitoring` | Monitoreo y telemetría | Prometheus, Jaeger |
| `AuditLogging` | Logging de auditoría | SIEM integration |

---

## Crear un Plugin

### Paso 1: Estructura del Proyecto

```bash
cargo new --lib my-plugin
cd my-plugin
```

### Paso 2: Configurar Cargo.toml

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
name = "my_plugin"
crate-type = ["cdylib"]  # IMPORTANTE: crea .so/.dylib/.dll
path = "src/lib.rs"

[dependencies]
synapsis-core = { path = "../../synapsis-core", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Paso 3: Implementar el Plugin

```rust
use std::sync::Arc;
use synapsis_core::domain::plugin::*;
use synapsis_core::domain::Result;

pub struct MyPlugin {
    info: PluginInfo,
}

impl MyPlugin {
    pub fn new() -> Self {
        Self {
            info: PluginInfo {
                id: "my-plugin".to_string(),
                name: "My Custom Plugin".to_string(),
                description: "Does amazing things".to_string(),
                version: "1.0.0".to_string(),
                author: "Your Name".to_string(),
                license: "MIT".to_string(),
                extension_points: vec![ExtensionPoint::Monitoring],
                dependencies: vec![],
            },
        }
    }
}

impl SynapsisPlugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn on_lifecycle(&self, lifecycle: PluginLifecycle) -> Result<()> {
        match lifecycle {
            PluginLifecycle::Load => eprintln!("[MyPlugin] Loading..."),
            PluginLifecycle::Initialize => eprintln!("[MyPlugin] Initializing..."),
            PluginLifecycle::Start => eprintln!("[MyPlugin] Starting..."),
            PluginLifecycle::Stop => eprintln!("[MyPlugin] Stopping..."),
            PluginLifecycle::Unload => eprintln!("[MyPlugin] Unloading..."),
        }
        Ok(())
    }

    fn extension_points(&self) -> Vec<ExtensionPoint> {
        self.info.extension_points.clone()
    }

    fn register_extensions(&self, registry: &mut PluginRegistry) -> Result<()> {
        // Registrar extensiones aquí
        registry.register_extension(
            ExtensionPoint::Monitoring,
            Arc::new(MyMonitor),
        );
        Ok(())
    }
}

// Macros requeridas para carga dinámica
synapsis_core::create_plugin!(MyPlugin);
synapsis_core::destroy_plugin!(MyPlugin);
```

### Paso 4: Construir el Plugin

```bash
# Debug
cargo build

# Release (recomendado para producción)
cargo build --release
```

El archivo resultante estará en:
- Linux: `target/release/libmy_plugin.so`
- macOS: `target/release/libmy_plugin.dylib`
- Windows: `target/release/my_plugin.dll`

---

## Cargar Plugins Dinámicamente

### Desde Código Rust

```rust
use synapsis_core::domain::plugin::{DynamicPluginLoader, PluginRegistry};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Crear loader y registry
    let mut loader = DynamicPluginLoader::new();
    let mut registry = PluginRegistry::new();
    
    // Cargar y registrar plugin
    loader.load_and_register("/path/to/my_plugin.so", &mut registry)?;
    
    println!("Loaded {} plugins", loader.loaded_count());
    
    // Usar registry con los plugins cargados
    // ...
    
    // Cleanup
    loader.unload_all()?;
    
    Ok(())
}
```

### Desde Synapsis MCP

```rust
use synapsis_core::domain::plugin::DynamicPluginLoader;

// En la inicialización del servidor MCP
let mut loader = DynamicPluginLoader::new();

// Cargar plugins desde un directorio
let plugin_dir = std::path::Path::new("~/.local/share/synapsis/plugins");

for entry in std::fs::read_dir(plugin_dir)? {
    let entry = entry?;
    let path = entry.path();
    
    if path.extension().map_or(false, |ext| {
        ext == "so" || ext == "dylib" || ext == "dll"
    }) {
        match loader.load_plugin(&path) {
            Ok(plugin) => {
                eprintln!("Loaded plugin: {}", plugin.info().name);
                // Registrar en el registry global
            }
            Err(e) => {
                eprintln!("Failed to load plugin {:?}: {}", path, e);
            }
        }
    }
}
```

---

## Ejemplo: Plugin de Criptografía

```rust
use std::sync::Arc;
use synapsis_core::domain::plugin::*;
use synapsis_core::domain::crypto::{CryptoProvider, PqcAlgorithm};
use synapsis_core::domain::Result;

pub struct CustomCryptoPlugin {
    info: PluginInfo,
    provider: Arc<CustomCryptoProvider>,
}

impl CustomCryptoPlugin {
    pub fn new() -> Self {
        Self {
            info: PluginInfo {
                id: "custom-crypto".to_string(),
                name: "Custom Crypto Provider".to_string(),
                description: "Hardware-backed cryptography".to_string(),
                version: "1.0.0".to_string(),
                author: "Security Team".to_string(),
                license: "Apache-2.0".to_string(),
                extension_points: vec![ExtensionPoint::CryptoProvider],
                dependencies: vec![],
            },
            provider: Arc::new(CustomCryptoProvider::new()),
        }
    }
}

impl SynapsisPlugin for CustomCryptoPlugin {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn on_lifecycle(&self, _lifecycle: PluginLifecycle) -> Result<()> {
        Ok(())
    }

    fn extension_points(&self) -> Vec<ExtensionPoint> {
        vec![ExtensionPoint::CryptoProvider]
    }

    fn register_extensions(&self, registry: &mut PluginRegistry) -> Result<()> {
        registry.register_extension(
            ExtensionPoint::CryptoProvider,
            self.provider.clone(),
        );
        Ok(())
    }
}

struct CustomCryptoProvider;

impl CryptoProvider for CustomCryptoProvider {
    fn id(&self) -> &str { "custom-hsm" }
    fn name(&self) -> &str { "Custom HSM Provider" }
    fn description(&self) -> &str { "Hardware security module backed" }
    fn version(&self) -> &str { "1.0.0" }
    
    fn supported_algorithms(&self) -> Vec<PqcAlgorithm> {
        vec![PqcAlgorithm::Kyber768, PqcAlgorithm::Aes256Gcm]
    }
    
    // Implementar métodos de CryptoProvider...
    fn generate_keypair(&self, _alg: PqcAlgorithm) -> Result<(Vec<u8>, Vec<u8>)> {
        // Implementación custom
        unimplemented!()
    }
    
    // ... resto de métodos
}

synapsis_core::create_plugin!(CustomCryptoPlugin);
synapsis_core::destroy_plugin!(CustomCryptoPlugin);
```

---

## Ciclo de Vida del Plugin

```
Load → Initialize → Start → [Running] → Stop → Unload
```

| Estado | Descripción | Cuándo ocurre |
|--------|-------------|---------------|
| `Load` | Plugin cargado en memoria | Al cargar el .so |
| `Initialize` | Dependencias disponibles | Después de registrar extensiones |
| `Start` | Listo para procesar | Cuando el servidor inicia |
| `Stop` | Deteniendo procesamiento | Antes de shutdown |
| `Unload` | Descargando de memoria | Al cerrar el servidor |

---

## Mejores Prácticas

### 1. Versionado

```toml
[package]
name = "my-plugin"
version = "1.2.3"  # SemVer

[dependencies]
synapsis-core = { version = "0.1", features = ["full"] }
```

### 2. Manejo de Errores

```rust
fn register_extensions(&self, registry: &mut PluginRegistry) -> Result<()> {
    // Validar configuración antes de registrar
    if !self.is_configured() {
        return Err(SynapsisError::new(
            ErrorKind::Validation,
            0x0301,
            "Plugin not properly configured"
        ));
    }
    
    registry.register_extension(...);
    Ok(())
}
```

### 3. Logging

```rust
use log::{info, warn, error};

fn on_lifecycle(&self, lifecycle: PluginLifecycle) -> Result<()> {
    match lifecycle {
        PluginLifecycle::Load => info!("Loading plugin: {}", self.info().name),
        PluginLifecycle::Start => info!("Starting plugin: {}", self.info().name),
        PluginLifecycle::Stop => info!("Stopping plugin: {}", self.info().name),
        _ => {}
    }
    Ok(())
}
```

### 4. Seguridad

⚠️ **ADVERTENCIA:** Los plugins cargan código dinámico. Solo cargar plugins de fuentes confiables.

```rust
// Verificar firma del plugin antes de cargar
fn verify_plugin_signature(path: &Path) -> Result<bool> {
    // Implementar verificación de firma
    Ok(true)
}

fn load_verified_plugin(path: &Path, loader: &mut DynamicPluginLoader) -> Result<()> {
    if !verify_plugin_signature(path)? {
        return Err(SynapsisError::new(
            ErrorKind::Security,
            0x0701,
            "Plugin signature verification failed"
        ));
    }
    loader.load_plugin(path)?;
    Ok(())
}
```

---

## Directorio de Plugins

Por defecto, Synapsis busca plugins en:

| Sistema | Ruta |
|---------|------|
| Linux | `~/.local/share/synapsis/plugins/` |
| macOS | `~/Library/Application Support/synapsis/plugins/` |
| Windows | `%APPDATA%\synapsis\plugins\` |

---

## Troubleshooting

### Plugin no carga

```bash
# Verificar dependencias compartidas
ldd libmy_plugin.so  # Linux
otool -L libmy_plugin.dylib  # macOS

# Verificar símbolos exportados
nm -D libmy_plugin.so | grep plugin_create  # Linux
nm -U libmy_plugin.dylib | grep plugin_create  # macOS
```

### Error de versión

Asegurar que el plugin y synapsis-core usen la misma versión:

```toml
# Plugin Cargo.toml
synapsis-core = { version = "0.1.0", features = ["full"] }

# synapsis-core Cargo.toml
version = "0.1.0"
```

---

## Recursos Adicionales

- **Ejemplo completo:** `synapsis-plugins-example/hello_plugin/`
- **API docs:** `cargo doc --open -p synapsis-core`
- **Plugin registry source:** `synapsis-core/src/domain/plugin.rs`
- **Dynamic loader source:** `synapsis-core/src/domain/plugin_loader.rs`
