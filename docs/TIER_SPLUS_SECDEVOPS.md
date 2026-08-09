# Tier S++ SecDevOps — Synapsis Ecosystem

**Estado:** Normativo  
**Alcance:** Todos los proyectos del ecosistema MethodWhite/Synapsis  
**Aplicación:** Synapsis, synapsis-core, Arca, x402-service, Noctua, Noctua-C, interfaces, servicios y herramientas relacionadas  
**Fuente:** Observaciones de seguridad del MCP Server de Synapsis (Obs. #219) y políticas de seguridad del ecosistema

> Este documento es la referencia canónica. Ningún proyecto puede declarar cumplimiento Tier S++ si contradice esta especificación.

## 1. Principios obligatorios

1. **Secure by default:** una configuración ausente o insegura debe detener el arranque, no activar un fallback peligroso.
2. **Zero Trust:** ningún cliente, producto, wallet, `user_id`, licencia o servicio remoto se considera confiable sin verificación.
3. **Least privilege:** cada proceso, token, wallet, endpoint y cuenta recibe solo los permisos necesarios.
4. **Fail closed:** ante una verificación incompleta, timeout, RPC caído, firma inválida o estado ambiguo, se rechaza la operación.
5. **Defensa en profundidad:** autenticación, autorización, validación, integridad, auditoría y rate limiting son capas independientes.
6. **Separación de productos:** Synapsis, Noctua y Noctua-C comparten protocolo, no datos, secretos ni catálogos de features por defecto.

## 2. Seguridad de secretos y configuración

- Prohibido guardar tokens, claves privadas, API keys, signing secrets o credenciales en Git.
- Prohibidos secretos por defecto en producción.
- Prohibida la dirección cero como wallet receptora.
- Las wallets, RPC endpoints privados y credenciales deben venir de variables de entorno o secret manager.
- Los logs nunca deben contener secretos, tokens completos, claves privadas ni payloads sensibles.
- `.env`, bases de datos locales, caches, entornos virtuales y artefactos de build deben estar excluidos por `.gitignore`.
- Todo secreto comprometido debe revocarse, rotarse y documentarse.

## 3. Criptografía e identidad

- Usar CSPRNG para IDs, tokens, nonces e invoices.
- Usar comparaciones de tiempo constante para secretos y firmas.
- Usar Ed25519 para licencias firmadas cuando corresponda.
- Usar HMAC o Standard Webhooks para autenticidad de webhooks.
- No usar hashes rápidos como autenticación.
- No inventar ni aceptar una identidad basándose solo en un ID enviado por el cliente.
- Validar expiración, audiencia, emisor y tipo de todos los tokens.

## 4. x402 y pagos on-chain

### Arca

Arca es la autoridad para wallet y settlement:

- Red y moneda explícitas: Base + USDC, salvo una especificación versionada distinta.
- Verificar receipt confirmado y estado exitoso.
- Verificar contrato oficial de USDC.
- Verificar evento ERC-20 `Transfer`.
- Verificar wallet receptora.
- Verificar pagador e importe mínimo.
- Rechazar transacciones reutilizadas.
- Persistir `tx_hash` de forma única.
- Usar timeout, retry limitado, backoff y circuit breaker para RPC.

### x402-service

Es la autoridad para cuentas y entitlements:

- Usuarios y API keys.
- Productos y features.
- Licencias y créditos.
- Ledger de uso.
- Entitlements y expiración.
- Idempotencia de eventos.
- Auditoría de concesión, consumo, revocación y reembolso.

No debe duplicar la verificación blockchain de Arca.

### Clientes

Synapsis, Noctua y Noctua-C:

- Declaran el producto y feature solicitado.
- Consumen el contrato x402 común.
- Aplican el entitlement recibido.
- No almacenan claves privadas del backend.
- No duplican la verificación on-chain.
- No desbloquean una feature ante una respuesta ambigua.

## 5. API y servicios

- Validar todos los inputs y limitar tamaño de requests.
- Rate limiting por IP, identidad, wallet, endpoint y operación sensible.
- Timeouts explícitos en todas las llamadas de red.
- Retries solo para errores transitorios, con límite y jitter.
- Circuit breaker para RPC, proveedores y servicios internos.
- CORS, TLS, headers de seguridad y exposición de puertos deben ser explícitos.
- Separar endpoints públicos, autenticados, administrativos y de webhook.
- Los endpoints administrativos requieren autenticación fuerte y autorización específica.
- Los webhooks deben verificar el body crudo antes de procesarlo.

## 6. Integridad, concurrencia y anti-replay

- Operaciones de saldo deben ser atómicas.
- Toda idempotencia debe estar respaldada por una restricción única en base de datos.
- `tx_hash`, `event_id`, `payment_id` y claves equivalentes no pueden procesarse dos veces.
- Los checks de existencia y las inserciones deben ejecutarse dentro de transacciones seguras.
- Los contadores y timestamps deben tener límites y validación.
- Los locks deben tener timeout y detección de deadlock cuando aplique.
- La corrupción o manipulación debe producir error visible y auditado.

## 7. Datos y privacidad

- Recoger y conservar solo datos necesarios.
- Separar datos por producto y entorno.
- No compartir licencias, créditos o cuentas entre productos sin autorización explícita.
- Definir retención y eliminación de datos.
- No incluir información personal en logs técnicos.
- Las bases locales y dumps no se suben al repositorio.
- Las migraciones deben ser reproducibles y verificadas en una base limpia.

## 8. Calidad de código

- Responsabilidad única y separación clara de capas.
- Interfaces/adaptadores para proveedores externos.
- Funciones pequeñas y nombres descriptivos.
- No duplicar protocolos, catálogos ni reglas de seguridad.
- Cambios de seguridad deben incluir tests de regresión.
- Documentar decisiones arquitectónicas relevantes.

## 9. Testing mínimo obligatorio

Cada proyecto debe ejecutar, según su tecnología:

- Tests unitarios.
- Tests de integración.
- Tests de autenticación y autorización.
- Tests de inputs inválidos.
- Tests de concurrencia y race conditions.
- Tests de replay/idempotencia.
- Tests de timeouts y fallos de dependencias.
- Tests de secretos ausentes y configuración insegura.
- Tests de migraciones desde una base limpia.
- Tests de build reproducible.

Para x402 además:

- Receipt inexistente.
- Receipt fallido.
- Red equivocada.
- Contrato USDC equivocado.
- Receptor equivocado.
- Importe insuficiente.
- Transferencia sin evento válido.
- `tx_hash` repetido.
- Dos solicitudes concurrentes para el mismo pago.

## 10. CI/CD y supply chain

Cada repositorio debe tener CI que incluya:

- Formateo.
- Linter con warnings tratados como error cuando sea viable.
- Tests.
- SAST.
- SCA y auditoría de dependencias.
- Detección de secretos.
- Revisión de licencias.
- SBOM cuando corresponda.
- Builds de release.
- Checksums y firma de artefactos de release.
- Dependabot/Renovate o proceso equivalente.

Herramientas base recomendadas:

- Rust: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo deny`, `cargo audit`.
- Python: `pytest`, `ruff`, `mypy` cuando aplique, `pip-audit`, detector de secretos.
- C/C++: compilación con warnings estrictos, sanitizers, `clang-tidy`, `cppcheck`, tests unitarios y de integración.

## 11. Threat model y respuesta a incidentes

Cada servicio que exponga red o procese pagos debe documentar:

- Activos protegidos.
- Actores y límites de confianza.
- Amenazas principales.
- Mitigaciones.
- Eventos auditados.
- Procedimiento de rotación de secretos.
- Procedimiento de revocación de licencias y entitlements.
- Procedimiento ante replay, doble gasto, RPC comprometido o fuga de credenciales.

Las vulnerabilidades deben reportarse de forma privada, nunca como issue público.

## 12. Definition of Done Tier S++

Un cambio no está terminado hasta que:

- [ ] El diseño respeta Zero Trust y fail-closed.
- [ ] No introduce secretos ni fallbacks inseguros.
- [ ] Tiene validación de inputs y autorización.
- [ ] Tiene tests de la ruta normal y de errores.
- [ ] Tiene protección contra concurrencia/replay si procesa estado o pagos.
- [ ] Tiene documentación actualizada.
- [ ] Pasa CI, SAST, SCA y detección de secretos.
- [ ] Se revisan migraciones y compatibilidad.
- [ ] Se registra el impacto de seguridad.
- [ ] Se verifica que no rompe otro proyecto del ecosistema.

## 13. Matriz inicial por proyecto

| Proyecto | Obligación principal | Estado inicial |
|---|---|---|
| Synapsis | Cliente/producto x402 y control de features | Requiere consolidar verificación con Arca |
| synapsis-core | Primitivas seguras y almacenamiento | Mantener políticas y CI |
| Arca | Wallet, Base/USDC, invoices y verificación | Requiere eliminar fallbacks y completar tests |
| x402-service | Usuarios, licencias, créditos y entitlements | Requiere quitar acoplamiento de producto y aplicar hardening |
| Noctua | Cliente/producto de análisis | Requiere contrato x402 común |
| Noctua-C | Core C y GUI C++ | Requiere cliente x402 seguro sin secretos en GUI |
| GUI/CLI | Presentación y operación | Nunca contiene claves privadas del backend |

## 14. Regla de cambios

Antes de modificar una pieza compartida, identificar consumidores y actualizar el contrato, tests y documentación correspondientes. No se permite resolver una integración creando otra implementación paralela del protocolo.


## 15. Pipelines CI/CD de altos estándares

Cada pipeline debe ser reproducible, auditable e inmutable en el tiempo:

- `on` restringido: ramas protegidas, tags firmados para release; sin triggers sobre `*` sueltos.
- `permissions` mínimas por job (`contents: read`; `security-events: write` y `packages: write` solo donde haga falta).
- `concurrency` con cancelación para evitar carreras.
- Acciones/versiones **pineadas** (tag o SHA completo); sin tags móviles no verificables.
- Secretos solo via secrets/CI de GitHub; fail-closed si faltan (no defaults en producción).
- Gates obligatorios antes de merge:
  - Format y lint (ruff/`cargo fmt`/eslint).
  - Type check (mypy/tsc).
  - Tests (pytest/vitest/flutter test) con cobertura mínima definida.
  - SAST (bandit/codeql), SCA (pip-audit/npm audit), secretos (gitleaks), SBOM (cyclonedx).
  - Escaneo de imagen (trivy) con severidad mínima y `ignore-unfixed`.
- Artefactos de release: checksums, firma, provenance/SLSA, y etiqueta inmutable (tag).
- Repositorios y dependencias Git privadas accesibles solo via token de lectura con permisos mínimos (`Contents: read`), nunca para escribir.
- Dependabot/Renovate con reglas para ignorar majors que rompan y revisión humana.
- No subir secretos, logs de debug ni artefactos intermedios a artefactos públicos.

## 16. Criptografía y datos en reposo (estándar actualizado)

- Cifrado autenticado con AEAD y nonce aleatorio por registro (sin modos inseguros: sin ECB, sin CBC sin MAC).
- KDF fuerte (Argon2id o PBKDF2 con iteraciones altas) para derivar claves desde secretos.
- Alternativa sin AES aceptada: ChaCha20-Poly1305 (construcción vetted). AES-GCM también es válido; el riesgo histórico suele ser implementación, no el algoritmo.
- Capa post-cuántica (ML-KEM / ML-DSA) donde el modelo de amenaza lo justifique, con implementaciones corregidas.
- Claves y credenciales de terceros (exchanges) cifradas en reposo y nunca devueltas por API.
- Fail-closed ante secreto inválido o datos corruptos.

## 17. Sistemas nuevos y faltantes del ecosistema (Arca Quant y SaaS)

- Motor de trading: backtesting con costos, walk-forward, datasets de aprendizaje, multi-agente con supervisor, ML pipeline con validación fuera de muestra.
- Broker agnóstico: interfaz común; adaptadores (Binance, Buda) firmados; modo paper/real con fail-closed; MockBroker para desarrollo sin cuentas.
- Conexión de exchanges: credenciales cifradas, validación de permisos, `GET/POST/DELETE /exchanges/*`; nunca exponer secretos.
- Multi-tenant: órdenes/posiciones aisladas por cuenta; admin con rol separado; sin escalada.
- Billing freemium/pro con límites por cuenta; pagos (Stripe) como adaptador futuro.
- Notificaciones: webhook firmado (HMAC), email (SMTP/STARTTLS), push (FCM); no-op seguro si no configurado.
- WebSocket autenticado para datos en vivo; CORS restringido; UI web y móvil con los mismos estándares (keystore, biometría, sin secretos en el cliente).
- Backups cifrados y restauración probada; kill switch global.

## 18. Observabilidad y operación

- Métricas, logs estructurados y traces en servicios expuestos.
- Health/readiness separados.
- Auditoría inmutable de acciones sensibles (trading, conexiones, billing, admin).
- Alertas de infraestructura y de riesgo (drawdown, límites de plan).
- Retención y rotación de logs definidas.

## 19. Matriz actualizada por proyecto

| Proyecto | Estado Tier S++ |
|---|---|
| Synapsis | CI verde; restan resolver dependencias transitivas y consolidar x402 |
| synapsis-core | Políticas y CI; revisar dependencias PQC |
| Arca | Clippy limpio; completar tests de settlement |
| Arca Quant | Backend 110+ tests; web y móvil con CI verde; conectores firmados; cripto sin AES |
| x402-service | Hardening pendiente; contrato de endpoints |
| Noctua | Contrato x402 común |
