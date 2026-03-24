# PQC Plugin Migration - Fase 1 Completada

## Resumen

Se consolidaron las implementaciones PQC duplicadas en un sistema de plugins unificado.

## Problema Original

El ecosistema MethodWhite tenía **3 implementaciones PQC duplicadas**:

1. **`synapsis/src/core/pqc.rs`** - Usando `pqcrypto` crate (Kyber512, Dilithium5)
2. **`pqc-packer/`** - Usando `pqcrypto` crate (Kyber768, Dilithium3)
3. **`rust/pqc-crypto/`** - Implementaciones from scratch (sin usar)

Esto violaba el principio DRY y dificultaba el mantenimiento.

## Solución Implementada

### 1. Nuevo `PqcryptoProvider` Unificado

**Archivo:** `src/core/pqcrypto_provider.rs`

Soporta **TODOS** los algoritmos PQC en un solo provider:

| Algoritmo | Tipo | Estado |
|-----------|------|--------|
| Kyber-512 | KEM | ✅ Implementado |
| Kyber-768 | KEM | ✅ Implementado |
| Kyber-1024 | KEM | ✅ Implementado |
| Dilithium-2 | Firma | ✅ Implementado |
| Dilithium-3 | Firma | ✅ Implementado |
| Dilithium-5 | Firma | ✅ Implementado |
| AES-256-GCM | Simétrico | ✅ Implementado |

### 2. CryptoPlugin Actualizado

**Archivo:** `src/core/crypto_plugin.rs`

Ahora registra **dos providers**:
- `PqcryptoProvider` - Provider primario (todos los algoritmos)
- `SynapsisPqcProvider` - Provider legacy (backward compatibility)

### 3. Errores Extendidos

**Archivo:** `src/domain/errors.rs`

Nuevos métodos de error:
- `crypto_pqc(msg)` - Error genérico PQC
- `crypto_cipher_msg(msg)` - Error de cifrado con mensaje

### 4. Dependencia Agregada

**Archivo:** `Cargo.toml`

```toml
argon2 = "0.5"  # Para derivación de claves desde password
```

## Arquitectura Resultante

```
┌─────────────────────────────────────────────────────────┐
│              Plugin System (SynapsisPlugin)             │
│  ┌───────────────────────────────────────────────────┐  │
│  │            CryptoPlugin                           │  │
│  │  ┌─────────────────┐  ┌──────────────────────┐   │  │
│  │  │ PqcryptoProvider│  │ SynapsisPqcProvider  │   │  │
│  │  │ (Primario)      │  │ (Legacy)             │   │  │
│  │  │ - Kyber 512/768 │  │ - Kyber512           │   │  │
│  │  │ - Kyber 1024    │  │ - Dilithium5         │   │  │
│  │  │ - Dilithium 2/3 │  │ - AES-256-GCM        │   │  │
│  │  │ - Dilithium 5   │  │                      │   │  │
│  │  │ - AES-256-GCM   │  │                      │   │  │
│  │  └─────────────────┘  └──────────────────────┘   │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Backward Compatibility

✅ **Totalmente compatible** - El código existente que usa `SynapsisPqcProvider` sigue funcionando.

## Próximos Pasos

### Fase 2: Auth System Plugin
- Crear `AuthProvider` trait
- Implementar plugins con crates establecidos (`jsonwebtoken`, `totp-rs`)
- Migrar 1982 líneas de código custom

### Fase 3: Task Queue Plugin
- Crear `TaskQueueProvider` trait
- Evaluar `background_job` crate
- Reemplazar 963 líneas custom

### Fase 4: synapsis-core Crate
- Extraer dominio + core business logic
- Sin DB, sin HTTP, sin MCP
- Publicar como crate reusable

### Fase 5: Plugin Registry Dinámico
- Carga de plugins .so/.dylib
- Hot-reload de capacidades
- Marketplace de plugins

## Beneficios Obtenidos

1. ✅ **DRY** - Una sola implementación por algoritmo
2. ✅ **Completo** - Todos los algoritmos PQC en un lugar
3. ✅ **Extensible** - Fácil agregar nuevos providers
4. ✅ **Backward Compatible** - Código legacy funciona
5. ✅ **Plugin-ready** - Base para sistema de plugins

## Archivos Modificados

| Archivo | Cambios |
|---------|---------|
| `src/core/pqcrypto_provider.rs` | Reescrito completamente |
| `src/core/crypto_plugin.rs` | Actualizado para usar ambos providers |
| `src/domain/errors.rs` | Agregados métodos de error |
| `Cargo.toml` | Agregado `argon2 = "0.5"` |
| `src/bin/mcp.rs` | Sin cambios (usa el plugin system) |

## Testing

```bash
# Verificar compilación
cargo check

# Build release
cargo build --release --bin synapsis-mcp

# Test tools MCP
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | \
  ./target/release/synapsis-mcp | jq '.result.tools | length'
# Expected: 52 herramientas
```

---

**Estado:** ✅ Fase 1 Completada
**Próxima Fase:** Auth System Plugin
