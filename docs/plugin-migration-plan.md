# Plugin System Migration Plan

## Problem Statement
The MethodWhite ecosystem has significant wheel reinvention, particularly in PQC cryptography where three separate implementations exist:
1. `synapsis/src/core/pqc.rs` - Uses `pqcrypto` crate (Kyber512, Dilithium5)
2. `pqc-packer/` - Uses `pqcrypto` crate (Kyber768, Dilithium3) 
3. `rust/pqc-crypto/` - From scratch implementations (unused)

Additionally, other areas show reinvention:
- **Auth System**: 1982 lines of custom code (fake TPM, RBAC, challenge-response, TOTP)
- **Task Queue**: 963 lines custom implementation vs established crates
- **Database Layer**: 1432 lines raw SQL with `rusqlite`, no ORM

## Solution: Modular Plugin Architecture
Implement a "Lego" plugin system similar to OpenVentus, where capabilities can be added externally to the core MCP server.

### Completed Work
1. **Plugin System Foundation** (`domain/plugin.rs`):
   - `SynapsisPlugin` trait with lifecycle management
   - `PluginRegistry` for managing plugins
   - Extension points for different capability types
   - Support for hybrid loading (static + dynamic .so/.dylib)

2. **CryptoProvider Abstraction** (`domain/crypto.rs`):
   - Unified trait for all PQC operations
   - Support for multiple algorithms (Kyber512/768/1024, Dilithium2/3/5, AES-256-GCM)
   - `CryptoProviderRegistry` for managing multiple providers

3. **Implementation Adapters**:
   - `SynapsisPqcProvider` - Adapter for existing `synapsis::core::pqc`
   - `PqcryptoProvider` - Comprehensive provider (can be extended to support all algorithms)
   - `CryptoPlugin` - Plugin wrapper for crypto providers

## Migration Steps

### Phase 1: Consolidate PQC Implementations (Priority: High)
1. **Extend `PqcryptoProvider`** to support all algorithm variants:
   - Add Kyber768 support (from `pqc-packer`)
   - Add Dilithium3 support (from `pqc-packer`)
   - Keep Kyber512 and Dilithium5 (from `synapsis::core::pqc`)

2. **Update existing PQC usage** in MCP server:
   - `src/presentation/mcp/secure_tcp.rs`
   - `src/presentation/mcp/server.rs`
   - Replace direct `crate::core::pqc::*` calls with `CryptoProvider` trait

3. **Deprecate redundant implementations**:
   - Mark `synapsis::core::pqc` as deprecated
   - Update `pqc-packer` to export `CryptoProvider` implementation
   - Archive `rust/pqc-crypto` (unused)

### Phase 2: Auth System Plugin (Priority: Medium)
1. **Create `AuthProvider` trait** in `domain/auth.rs`:
   ```rust
   pub trait AuthProvider: Send + Sync {
       fn authenticate(&self, credentials: &AuthCredentials) -> Result<AuthToken>;
       fn authorize(&self, token: &AuthToken, permission: &str) -> Result<bool>;
       fn create_user(&self, user: &User) -> Result<()>;
       // etc.
   }
   ```

2. **Implement plugins using established crates**:
   - `JwtAuthPlugin` using `jsonwebtoken`, `argon2`
   - `TotpAuthPlugin` using `totp-rs`
   - `RbacAuthPlugin` using established RBAC libraries

3. **Migrate 1982 lines of custom auth code**:
   - Preserve business logic but replace crypto/primitives
   - Maintain compatibility with existing sessions

### Phase 3: Task Queue Plugin (Priority: Medium)
1. **Create `TaskQueueProvider` trait**:
   - Support for `background_job`, `sqlx-queue`, or custom implementations

2. **Replace 963 lines of custom task queue**:
   - Evaluate `background_job` crate compatibility
   - Create adapter or migrate fully

### Phase 4: Database Abstraction (Priority: Low)
1. **Create `DatabaseProvider` trait**:
   - Support for `sqlx`, `diesel`, or raw SQLite

2. **Wrap 1432 lines of raw SQL**:
   - Incremental migration to ORM
   - Maintain SQLite compatibility for embedded use

### Phase 5: Plugin Ecosystem (Priority: Low)
1. **Create `synapsis-core` crate**:
   - Minimal core with plugin API
   - Shared types and traits

2. **Establish plugin registry**:
   - Internal plugins (developed by MethodWhite)
   - Enterprise plugins (customer-specific)
   - Community plugins (open source)

## Technical Implementation Details

### Plugin Loading Strategy
```rust
// Hybrid loading: static features + dynamic libraries
let mut registry = PluginRegistry::new();

// Static plugins (compiled-in)
registry.register_plugin(Arc::new(CryptoPlugin::new()));

// Dynamic plugins (.so/.dylib/.dll)
#[cfg(unix)]
let plugin_lib = libloading::Library::new("auth_jwt.so")?;
// Use libloading to get plugin entry point
```

### Extension Points
Based on existing patterns in codebase:
- `ExtensionPoint::CryptoProvider` - For `CryptoProvider` implementations
- `ExtensionPoint::AuthProvider` - For authentication systems  
- `ExtensionPoint::StorageBackend` - For `StoragePort` implementations
- `ExtensionPoint::LlmProvider` - Existing `LlmProvider` trait
- `ExtensionPoint::WorkerAgent` - Existing `WorkerAgent` trait
- `ExtensionPoint::RpcHandler` - Existing `RpcHandler` trait

### Backward Compatibility
1. **Feature flags** for gradual migration:
   - `plugin-crypto` - Use plugin system for crypto
   - `plugin-auth` - Use plugin system for auth
   - `legacy-pqc` - Keep old `pqc` module active during transition

2. **Adapter patterns** to bridge old and new:
   - `LegacyPqcAdapter` implements `CryptoProvider` for old code
   - `PluginAuthAdapter` implements old auth traits for plugins

## Coordination with Other Agents
The Synapsis MCP task system (port 7438, JSON-RPC 2.0) should be used to:
1. Assign specific audit/implementation tasks to `pqc-worker-*` agents
2. Coordinate testing with `pqc-tester-*` agents  
3. Optimize performance with `pqc-optimizer-*` agents

Current status: 8 audit tasks assigned to PQC agents are still `running`.

## Immediate Next Actions
1. **Complete PqcryptoProvider implementation**:
   - Add Kyber768 support using `pqcrypto_kyber::kyber768`
   - Add Dilithium3 support using `pqcrypto_dilithium::dilithium3`
   - Test with existing `pqc-packer` test vectors

2. **Create migration example**:
   - Update `secure_tcp.rs` to use `CryptoProvider`
   - Demonstrate plugin registration in main application

3. **Coordinate with other agents**:
   - Check status of PQC audit tasks
   - Delegate algorithm implementation tasks

## Benefits
1. **Eliminate wheel reinvention** - Single implementation per capability
2. **Modular architecture** - Mix and match components like Lego
3. **Enterprise ready** - Customers can add proprietary plugins
4. **Community ecosystem** - Open source plugins for extended functionality
5. **Maintainability** - Smaller core, clear boundaries, established crates

## Files Created/Modified
- `src/domain/crypto.rs` - CryptoProvider trait and registry
- `src/domain/plugin.rs` - Plugin system foundation  
- `src/core/crypto_provider.rs` - Adapter for existing PQC
- `src/core/crypto_plugin.rs` - Plugin wrapper for crypto
- `src/core/pqcrypto_provider.rs` - Comprehensive provider
- `src/domain/mod.rs` - Updated exports
- `src/core/mod.rs` - Updated module declarations