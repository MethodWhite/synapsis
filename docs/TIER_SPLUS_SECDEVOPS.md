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

## 20. Gobernanza

### 20.1 Roles y responsabilidades

- **Responsable de seguridad del ecosistema (Security Owner):** autoridad final sobre
  excepciones, revocación de secretos y evaluación de riesgos cross-proyecto.
- **Maintainer por proyecto:** responsable del DoD, revisión de cambios y del threat
  model de su proyecto.
- **Revisor (reviewer):** puede aprobar PRs de código no sensible; los cambios de
  seguridad requieren revisión del Security Owner o un maintainer delegado.
- **Autor del cambio:** responsable de documentar impacto, tests y compatibilidad.

### 20.2 Proceso de aprobación

- Todo cambio de seguridad (auth, cripto, pagos, secretos, fail-closed) requiere:
  1. PR con referencia al control Tier S++ que toca.
  2. Al menos una revisión humana (dos si toca cripto o pagos).
  3. CI verde (lint, tests, SAST, SCA, secretos).
  4. Registro del impacto de seguridad en la descripción del PR.
- Ningún miembro aprueba su propio PR de seguridad.
- Excepciones: documentadas con justificación y fecha de revisión; nunca silenciosas.

### 20.3 Regla de cambios compartidos

- Antes de tocar una pieza compartida (protocolo, catálogo, primitivas seguras):
  - Identificar consumidores (§14).
  - Actualizar contrato, tests y documentación en el mismo PR.
  - Verificar que no rompe otros proyectos del ecosistema (matriz §13/§19).
- Prohibido crear implementaciones paralelas del protocolo para evitar integración.

### 20.4 Responsabilidad de cambios de seguridad

- CVE/incidente: el Security Owner coordina; el proyecto afectado publica el análisis
  y la mitigación en privado (SECURITY.md), nunca como issue público.
- Rotación de secretos: documentada, con fecha límite y verificación de revocación.
- Cambios de hardening: se registran con el control que satisfacen y el método de
  verificación (build, test, auditoría).

### 20.5 Métricas de gobernanza

- % de PRs de seguridad con revisión humana = 100%.
- % de excepciones documentadas = 100% (ninguna silenciosa).
- DoD cumplido en cada merge (verificable vía checklist del PR).
- Auditoría de gobernanza ejecutable en CI (módulo secdevops-audit).

## 21. Diseño y UX segura

### 21.1 Principios de diseño seguro

- **Secure UX:** el diseño hace que el usuario haga lo seguro por defecto; las
  decisiones de seguridad visibles (permisos, consentimiento, advertencias) usan
  patrones comprensibles, nunca engañosos.
- **Fail-closed visible:** ante un error de seguridad, la UI muestra estado claro
  y acción de recuperación; nunca silencia un riesgo (dark patterns prohibidos).
- **Menor sorpresa:** la interfaz no oculta acciones destructivas ni exige
  confirmaciones ambiguas; confirmación explícita para operaciones irreversibles.
- **Defensa en profundidad en UX:** validación en UI y backend; la UI nunca es
  el único control de seguridad.

### 21.2 Usabilidad (ISO 9241 / ISO 25010)

- Efectividad, eficiencia y satisfacción medibles (no solo estética).
- Protección contra errores del usuario (undo, confirmación, validación inline).
- Consistencia de patrones y terminología; evitar jerga técnica innecesaria.
- Onboarding y documentación accesibles dentro del producto.

### 21.3 Accesibilidad (WCAG 2.2)

- Perceptible, operable, comprensible y robusto.
- Nivel AA mínimo obligatorio para interfaces de usuario.
- Soporte de teclado, lectores de pantalla, contraste y alternativas textuales.
- Modo claro/oscuro sin pérdida de legibilidad.

### 21.4 Seguridad de la interfaz

- No exponer secretos, tokens ni información sensible en la UI.
- Mensajes de error sin revelar detalles internos (no stack traces al usuario).
- Logs de UI sin PII innecesaria.
- Rate limiting y validación en la UI donde aplique (sin confiar solo en cliente).

## 22. Stakeholders y producto

### 22.1 Análisis de interesados

- Identificar y documentar stakeholders (usuarios finales, operadores, seguridad,
  negocio, reguladores, otros proyectos del ecosistema).
- Registrar su influencia, expectativas y requisitos en un registro de interesados.
- Revisar el registro en cada hito; actualizar ante cambios de alcance.

### 22.2 Requisitos

- Requisitos funcionales y no funcionales (incl. seguridad, usabilidad, accesibilidad,
  rendimiento) trazables y verificables.
- Los requisitos de seguridad se tratan como requisitos de primera clase, con
  criterio de aceptación y método de verificación.
- Priorización explícita (MoSCoW o equivalente) documentada.

### 22.3 Producto y roadmap

- Roadmap con hitos verificables y criterios de done por release.
- Las features se evalúan contra el modelo de amenaza (§11) y el DoD (§12).
- Retroalimentación de usuarios y stakeholders incorporada en el ciclo.
- Cambios de alcance aprobados y documentados (§20.2, §20.3).

### 22.4 Comunicación

- Canales definidos para reportar vulnerabilidades (privado) y para feedback de
  producto.
- Transparencia sobre capacidades, límites y estado de seguridad del producto.

## 23. Modelado de amenazas y testing (MITRE ATT&CK / OSSTMM)

### 23.1 MITRE ATT&CK

- El threat model (§11) debe mapear sus amenazas a las tácticas y técnicas de
  MITRE ATT&CK (enterprise, mobile, ICS según el dominio).
- Cada mitigación se asocia a la técnica ATT&CK que neutraliza, y cada control a
  la técnica que detecta o bloquea.
- Las pruebas de seguridad (red team, pentest, detección) se diseñan contra
  técnicas ATT&CK, no solo contra CVE.
- Los casos de detección/telemetría se validan contra técnicas relevantes del
  threat model.

### 23.2 OSSTMM

- Las auditorías de seguridad se estructuran siguiendo OSSTMM:
  - Análisis de seguridad (postura, visibilidad, acceso, confianza, cumplimiento).
  - Validación de los canales aplicables (humanos, físicos, inalámbricos,
    telecomunicaciones, redes de datos).
  - Verificación de controles (interactivos, de proceso, de contenido,
    criptográficos, de validación).
- Las métricas usan el modelo RAV (Risk Assessment Values): RA, SEV, TRV, CV.
- Toda auditoría es verificable, repetible y con alcance documentado.
- Los hallazgos se priorizan por impacto y se vinculan al control Tier S++ y a
  la técnica ATT&CK que exponen.

### 23.3 Integración con el pipeline

- SAST, SCA y detección de secretos (§10) se complementan con pruebas dinámicas
  y de threat model (§23.1) y con auditoría estructurada (§23.2).
- Los hallazgos de testing alimentan el registro de riesgos y el DoD (§12).

## 24. Plataformas SaaS (aprendizaje aplicado)

### 24.1 Arquitectura

- Separación frontend/backend: SPA con shell persistente (sidebar+topbar no se recargan, solo el main content).
- Layout raíz: Sidebar (logo, navegación, footer) + Topbar (breadcrumbs, búsqueda, theme toggle) + main content.
- Componentes reutilizables: Card, DataTable, Badge, Button, Input, Dialog, Toast, Skeleton, Chart.
- API REST consumida por el frontend; datos vía fetch con estados loading/error/empty diseñados.

### 24.2 Seguridad del SaaS (obligatorio)

- **Autenticación**: token de sesión (secrets.token_urlsafe) con comparación en tiempo constante (hmac.compare_digest). Todas las APIs exigen el token (401 sin él).
- **Bind por defecto a 127.0.0.1**; si se expone en red (--host 0.0.0.0), advertir sobre TLS.
- **CSRF**: validar header custom (X-Requested-With/token); GET nunca cambia estado.
- **XSS**: escapar todo dato de usuario interpolado en innerHTML (helper esc()); CSP (default-src 'self') + X-Frame-Options: DENY + X-Content-Type-Options: nosniff.
- **Limits**: Content-Length cap (1MB), máx. sesiones simultáneas, rate limiting por IP.
- **TLS** para producción; cert autofirmado generado una vez.

### 24.3 UI/UX

- **KPI cards**: grid de cards (label pequeño, valor grande, delta, sparkline).
- **Data tables densas**: texto sm, borders sutiles, hover, badges de estado, font-mono para IDs/hashes.
- **Dark mode** obligatorio: tokens CSS en :root/.dark, toggle 3 estados (light/dark/system), sin FOUC.
- **Colores semánticos**: primary, success (emerald), warning (ámbar), destructive (rojo), chart-1..5.
- **Paleta**: base zinc/slate; tipografía Inter + mono para datos técnicos.
- **Responsive**: sidebar→overlay, KPIs→stack, tablas→scroll.

## 25. CLI y TUI moderno

### 25.1 CLI

- Estructura `sustantivo verbo`; flags largos estándar (-h/--help, --json, --plain, --no-color, --no-input, --dry-run, --version).
- stdout = datos (pipeable, --json); stderr = mensajes/errores. Nunca mensajes a stdout.
- Exit codes: 0 éxito, 1 runtime, 2 uso/parseo.
- Autocompletado nativo; help con ejemplos primero.
- Colores: respetar NO_COLOR, TERM, FORCE_COLOR, TTY detection (Rich lo maneja).
- Progress/spinner solo si TTY; nunca prompt si stdin no es TTY.
- Secrets solo por archivo/stdin, nunca en flags/env.
- Config precedencia: flags > env > archivo > defaults (XDG via platformdirs).

### 25.2 TUI (Textual/BubbleTea)

- Arquitectura reactiva: Model/App con estado, eventos, vistas declarativas.
- Widgets: DataTable (sort/filtro/scroll), ListView, Input, Tabs, ProgressBar, LoadingIndicator.
- Dark mode adaptativo (detectar fondo del terminal).
- Keybindings declarativos (footer automático); vim-style.
- Logging a stderr o archivo (nunca stdout en TUI).

## 26. GUI de escritorio

- **Recomendación**: Tauri 2.0 (Rust core + webview) con motor Python como sidecar (PyInstaller). Bundle pequeño, bajo RAM, seguridad por capabilities + CSP.
- Alternativa Python-pura: PySide6/Qt (widgets nativos, QtCharts, QTermWidget).
- No recomendado: Electron (pesado, RAM) ni Kivy (UI pobre para datos densos).
- Patrones: sidebar + multi-panel redimensionable, dark mode, tablas virtualizadas, monitorización por eventos (streaming, no polling), terminal integrada (xterm.js/QTermWidget), charts (ECharts/QtCharts).
- Screenshot del emulador: base64 data-URL o asset protocol, actualizado por evento.
- Empaquetado: Tauri → AppImage/deb/rpm (Linux), MSI/NSIS (Win), DMG (macOS), con firma.

## 27. AML y SARS (Suspicious Activity Report)

### 27.1 Flujo regulatorio (FinCEN)

- Detonadores: structuring/smurfing, layering, funnel accounts, velocity spikes, out-of-pattern.
- Thresholds configurables por segmento (no umbral único).
- Flujo: detección → triage → investigación (two-eyes para alto riesgo) → decisión → SAR.
- Timeline: Day 0 detección, Day 30 filing inicial, Day 120 periodo, Day 150 continuing activity.
- Campos SAR (Form 111): Part I subject info, Part II suspicious activity (amount, date, items 32-41 tipos, cyber indicators 42-44), Part III institución, Part V narrative.
- Recordkeeping: retener evidencia 5 años.

### 27.2 Dashboard AML

- KPIs: volumen monitoreado, alertas, casos abiertos, SARs filed, tasa falsos positivos.
- Transaction/event table: fecha, sujeto, contraparte, tipo, monto, score, status badge.
- Status lifecycle: monitoring → alert → under_review → cleared | escalated | reported.
- Risk score visual 0-100 color-coded (verde/ámbar/rojo), explicable (risk factors loggeados).
- Case management: alert queues, auto-assignment, SLA, dispositions, audit trail.
- Network graph: unifica users/devices/IPs/contrapartes para ver rings coordinados.

### 27.3 Fusión con emulación de dispositivos

- Evento del dispositivo emulado (boot, syscall, network, login) = transacción AML.
- Sujeto = identidad del dispositivo (fingerprint, IP, MAC); contraparte = servicio accedido.
- Tipologías: structuring (accesos bajo threshold), layering (rotación IPs), funnel (bursts), device-intelligence flags.
- Arquitectura: emulador → eventos JSON → SarsEngine (reglas/scoring) → alertas → dashboard → SAR generator (narrativa auto) → export PDF/BSA.

## 28. Desarrollo operacional seguro (DevSecOps)

### 28.1 Auditoría de seguridad

- **Secretos**: gitleaks en CI; sin tokens/keys en código ni historial git.
- **Auth/CSRF/XSS** en cualquier interfaz web (ver §24.2).
- **Fail-closed**: set -euo pipefail en scripts; except: pass prohibido (deben loggear); subprocess con check= o manejo de returncode.
- **Backups**: todo auto-fix debe crear backup antes de modificar (ej. *.remedy.bak), abortar si falla.
- **Temp files**: tempfile.mkdtemp() con O_NOFOLLOW; nunca paths predecibles en /tmp.
- **Inyección**: validar inputs contra allowlists (nunca concatenar raw a comandos); sin shell=True/eval/exec.
- **pkill/pgrep**: nunca por substring sin verificación de binario/usuario.

### 28.2 CI/CD mínimo (verificable)

- GitHub Actions: ruff, shellcheck, gitleaks, bandit, pip-audit.
- Tests pytest mínimos; release job con sha256sum.
- El auditor (secdevops-audit) debe ejecutar ruff check real (no solo which).

### 28.3 Endurecimiento del dashboard

- Logging estructurado (sin PII en logs).
- Audit trail para acciones AML.
- No exponer paths internos del entorno en scripts (PYTHON hardcodeado).

## 29. Enforcements y gobernanza de CI/CD (auditoría P0)

### 29.1 El CI DEBE BLOQUEAR (fail-closed, no continue-on-error)

- Ningún check de seguridad puede usar `continue-on-error` para pasar: CodeQL,
  gitleaks, bandit/pip-audit, cppcheck, ruff, shellcheck, tests y build DEBEN
  fallar el merge cuando detectan un hallazgo real.
- `exit 0` falso prohibido: un job que no corre el análisis real no puede
  reportar éxito (p. ej. el auditor debe ejecutar `ruff check` de verdad, no
  `which ruff`).
- Branch protection + rulesets: `main` no recibe push directo; PR obligatorio
  con checks verdes; los cambios a `.github/workflows/*`, `SECURITY.md` y
  `TIER_SPLUS_SECDEVOPS.md` requieren 2 reviews humanos (nadie auto-mergea
  infraestructura de seguridad).
- Auto-merge de dependencias solo con los checks verdes del PR (nunca mergear
  antes de que el CI confirme build+test+scan).

### 29.2 Supply chain reproducible

- Python: lockfile (uv/poetry/pip-tools) con hashes commitado; `pip-audit` en CI.
- C/C++: dependencias pinneadas por tag/rev (no `--depth 1` a HEAD móvil);
  vendoring de deps críticas (argtable3, microhttpd) con checksum verificado.
- Descargas externas (jadx, linuxdeploy, chromium, fuentes YARA): verificar
  SHA-256 del artefacto, no solo el tarball.
- SBOM: generar (CycloneDX/SPDX) en cada release y adjuntarlo al release; el
  CI lo regenera automáticamente (anchore/sbom-action o syft).
- Dependabot/Renovate cubriendo TODAS las dependencias (incluidas C/C++ vía
  vcpkg/conan o el manifest propio), con auto-PR que espera checks verdes.

### 29.3 Integridad de releases

- Tags firmados (anotados + GPG); la firma no puede ser condicional a un
  secreto con `continue-on-error`.
- Binarios firmados + checksums.sha256 en cada release.
- Provenance: SLSA provenance (attestation) generado por el workflow de
  release (actions/attest-build-provenance) para trazabilidad build→artefacto.
- Changelog obligatorio en el release; `VERSION` bump con Conventional Commits.

### 29.4 Testing avanzado en CI

- Fuzzing: target de fuzzing (libFuzzer/AFL) corriendo en CI con corpus mínimo
  (p. ej. 60s por PR o nightly) sobre los parsers críticos (ELF, PE, DEX, pcap,
  configs cifradas).
- Coverage gate: `make coverage`/pytest --cov con umbral configurable (p. ej.
  ≥70% en módulos nuevos); el release no procede bajo el umbral.
- DAST: para los daemons REST (noctua_rest_api, FastAPI), smoke HTTP de los
  endpoints con auth/token, SSRF checks, inyección básica.

## 30. Contrato de módulos y API estable (benchmark Rizin/SQLite/Binary Ninja)

### 30.1 API C estable y versionada

- `libnoctua` exporta una API C pública estable y versionada (`libnoctua.so.1`)
  con un test de ABI (tipo SQLite) que falla el CI si un símbolo público cambia.
- Funciones públicas con prefijo `noctua_`, structs opacos, flags de error
  consistentes (ver `noctua_err_str`).
- El core nunca depende de la interfaz: CLI/TUI/GUI/REST/bindings son capas
  delgadas sobre `libnoctua` (regla ya cumplida en Noctua-C — blindarla).

### 30.2 JSON canónico y determinista por módulo

- Cada módulo produce un JSON **canónico** (claves ordenadas, sin timestamps
  aleatorios, tipos estables) tanto en C como en Python — misma salida para la
  misma entrada en ambos ports (mirror).
- Patrón rizin `-j` / Volatility: la salida JSON ES el contrato de datos; el
  texto/HTML/PDF son vistas derivadas.
- Test de golden files: comparar el JSON de cada módulo entre el port C y el
  port Python; divergencia = fallo del mirror.

### 30.3 Cadena de custodia en reportes notariales

- Además del PDF notarial, generar `evidence.json` canónico (hashes, fases,
  decisiones) + `custody.json` (quién/quién accedió/cuándo) + timestamp
  RFC3161 (TSA) para que el PDF sea solo una vista reproducible de datos
  verificables.
- El verify.sh del notarize valida evidencia + firma + timestamp, no solo la
  firma.

## 31. Seguridad de IA, LLM y agentes (Secure AI)

### 31.1 Riesgos propios de IA/LLM

- **Prompt injection** (directa e indirecta): el contenido de entrada externo
  nunca debe poder reescribir las instrucciones del sistema. Aislamiento de
  instrucciones, separación de datos no confiables, detectores/guardas.
- **Tool poisoning**: un modelo con acceso a herramientas (ejecución, RAG,
  API) puede ser manipulado. Whitelist de herramientas, permisos mínimos,
  confirmación humana para acciones destructivas.
- **Excesiva agencia**: el agente no debe poder hacer más de lo necesario.
  Capability-based (reutilizar §5/§28), sandbox para ejecución.
- **Envenenamiento de modelos/RAG**: datos de entrenamiento o RAG corruptos.
  Provenance del corpus, verificación de fuentes, inmutabilidad de vectores.
- **Data exfiltration vía contexto**: el modelo puede filtrar secretos o
  datos sensibles del contexto. Redacción de secretos antes de enviar,
  minimización de contexto.
- **Alucinaciones en decisiones de seguridad**: nunca usar la salida del LLM
  como único control de seguridad (fail-closed, ver §1.4).

### 31.2 OWASP LLM Top 10 (2025) como checklist

- LLM01 Prompt injection; LLM02 Sensitive information disclosure; LLM03 Supply
  chain; LLM04 Data and model poisoning; LLM05 Improper output handling;
  LLM06 Excessive agency; LLM07 System prompt leakage; LLM08 Vector and
  embedding weaknesses; LLM09 Misinformation; LLM10 Unbounded consumption.
- Para cada técnica: mitigación, test (red team de prompts), telemetría.

### 31.3 EU AI Act / riesgo

- Clasificar el uso de IA (riesgo inaceptable/alto/limitado/mínimo) según EU
  AI Act; el análisis automatizado de binarios/side-channel es de riesgo
  limitado/minimo → transparencia y documentación técnica.
- Registro de modelos, datos de entrenamiento, límites de uso documentados.
- Evaluación de sesgo y robustez adversarial.

### 31.4 Gobernanza de agentes

- Todo agente (MCP server, subagente, autónomo) tiene: identidad, permisos,
  límites de recursos, logging completo, y kill-switch.
- Los agentes que modifican archivos siguen el mismo integrity gate que el
  remedy (§0 de tools/remedy): solo archivos pristinos, backup antes.
- MITRE ATLAS para modelar amenazas adversariales de ML/agentes.

## 32. Respuesta a incidentes y playbooks (NIST SP 800-61 Rev. 3)

### 32.1 Fases

- **Preparation**: equipo, herramientas, contactos, runbooks, canal seguro.
- **Detection & Analysis**: detección (EDR/audit), triage, análisis de
  impacto, cadena de custodia.
- **Containment, Eradication & Recovery**: aislamiento, remoción, restauración
  verificada, hardening post-incidente.
- **Post-Incident Activity**: lecciones aprendidas, informe, métricas, DoD.

### 32.2 Playbooks (RB-*)

- Un playbook por tipología: breach, ransomware, secret leak, supply-chain
  compromise, abuso de API, incidente de IA/agente, incidente AML.
- Cada playbook: detonadores, severidad (SSVC), acciones paso a paso, dueños,
  plazos, escalación.
- Los playbooks viven en `standards/runbooks/` (ver repo de estándares).

### 32.3 Notificación legal (plazos)

- GDPR: 72 h a la autoridad; NIS2: alerta temprana 24 h + notificación 72 h;
  DORA: a la autoridad competente; CRA: a ENISA y a la autoridad nacional.
- Modelo de incidentes con estados y fechas; evidencia en `evidence/`.

## 33. Cumplimiento y privacidad (CRA/DORA/NIS2/GDPR)

### 33.1 Marcos aplicables según dominio

- **CRA (Cyber Resilience Act)** para productos digitales en la UE: requisitos
  de seguridad desde diseño, SBOM, reporte de vulnerabilidades y explotación
  activa a ENISA.
- **DORA** para el sector financiero: resiliencia operativa digital, pruebas
  de resiliencia (TLPT), gestión de TPP.
- **NIS2** para operadores esenciales: gestión de riesgos, cadena de suministro,
  reporte de incidentes.
- **GDPR art. 25** (privacy by design/default): minimización, seudonimización,
  DPIAs para procesamiento de alto riesgo.
- **MiCA / Travel Rule** para activos on-chain (integrar con §4 x402).

### 33.2 Privacy by design

- Minimización de datos por defecto; PII nunca en logs ni en JSON canónico
  salvo requerimiento; retención con expiración.
- DSAR (right to access/delete) operativo: export y borrado verificable.
- DPIAs registrados en `evidence/` cuando aplique.

## 34. Gestión de vulnerabilidades priorizada (EPSS / CISA KEV / CVSS 4.0)

### 34.1 Priorización (no solo contar)

- Todo hallazgo SAST/SCA/fuzz se prioriza con: CVSS 4.0 + **EPSS**
  (probabilidad de explotación) + **CISA KEV** (explotación activa conocida)
  + contexto local (exposición, reachability).
- Los hallazgos en CISA KEV con EPSS alto se tratan como P0 (SLA de horas).
- SSVC (Stakeholder-Specific Vulnerability Categorization) para decisiones de
  prioridad y tiempo.

### 34.2 VEX y avisos

- Emitir **VEX** (Vulnerability Exploitability eXchange) por cada release:
  qué vulnerabilidades del SBOM aplican, cuáles no explotables, workarounds.
- Aviso de seguridad público (SECURITY.md / advisory) con severidad, CVSS,
  EPSS, KEV status, mitigación, timeline.

### 34.3 SLAs de remediación

- SLA por severidad (P0 horas, P1 días, P2 semanas) configurable por proyecto;
  el CI bloquea si un P0 supera el SLA sin excepción documentada.

## 35. Zero Trust operativo (NIST SP 800-207 / ZTNA)

### 35.1 Principios desplegados (no declarativos)

- **Nunca confiar, siempre verificar**: cada acceso autenticado y autorizado
  con contexto (identidad, dispositivo, red, riesgo).
- **Workload identity**: cada servicio (Noctua API, Arca, x402) tiene
  identidad propia (SPIFFE/mTLS o token corto) — no comparte credenciales.
- **Microsegmentación**: acceso por policy, no por ubicación de red; deny por
  defecto entre servicios.
- **Continuous verification**: re-validación de sesión/contexto, no solo
  login; anomalía → challenge.

### 35.2 ZTNA / access

- Los dashboards y APIs se exponen solo vía gateway ZTNA (o bind 127.0.0.1 +
  tunnel) — nunca red abierta (§24.2).
- Dispositivos/gente con identidad verificada antes del acceso; sin
  confianza implícita de VPN legacy.

### 35.3 Telemetría y auditoría

- Logs de acceso con decisión (allow/deny + motivo), sujeto, destino, contexto.
- Monitoreo continuo de desvíos de policy; alertas en tiempo real.
