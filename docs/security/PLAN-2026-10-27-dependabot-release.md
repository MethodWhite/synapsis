# Plan de remediación de seguridad y release de Synapsis

**Estado:** remediación local validada; integración y publicación pendientes  
**Repositorio:** `MethodWhite/synapsis`  
**Rama de trabajo:** `codex/synapsis-security-release-20261027`  
**Línea base revisada:** `main` en `25bab13` (v0.13.1)  
**Fecha:** 2026-10-27

## 1. Objetivo y alcance

Remediar las alertas abiertas de Dependabot del SDK `packages/synapsis-client`, integrar la corrección vigente de Quinn/RustSec que ya está propuesta para `main`, mantener el escaneo OSV en las ramas de publicación y desarrollo, validar el resultado sin relajar el acceso a dependencias privadas y producir un release cuyo commit/tag contenga las correcciones.

La revisión encontró seis alertas Dependabot abiertas, todas para Vite `8.3.2` en el lockfile del SDK y marcadas como dependencias de desarrollo. El PR #83 propone Vite `8.3.5`. Las seis advisories consideran afectada la línea `8.3.0–8.3.2` y fijada desde `8.3.3`; `8.3.5` supera esa versión. El riesgo es de herramientas de desarrollo/build, no evidencia de una vulnerabilidad explotable en el SDK publicado. La exposición concreta depende de las condiciones descritas por cada advisory.

La auditoría actual de `Cargo.lock` añadió hallazgos RustSec posteriores a los PR revisados: Tokio, rustls, rustls-webpki, raíces WebPKI y componentes criptográficos. Se fijaron versiones corregidas para todas las vulnerabilidades; `crypto-bigint 0.7.6` queda como advertencia yanked de Cargo porque es la versión upstream que RustSec señala como corregida. La ruta que la incorpora es opcional (`arca` → K-256); sin `arca`, no forma parte del grafo activo. No se añadió ningún ignore. Esto queda como riesgo documentado que requiere CI y revisión humana antes del tag.

El paquete no tenía archivos de prueba aunque declaraba `vitest run`; se añadieron tres pruebas unitarias acotadas al contrato existente de `SynapsisClient.callTool` (JSON-RPC, cabecera de autenticación y errores) para que `npm test` tenga una señal real durante esta actualización.

También hay un PR de seguridad Quinn para `main` (#79), con `quinn 0.11.12` y `quinn-proto 0.11.19`, aprobado y con CI verde al iniciar esta revisión. El PR #82 lleva remediaciones equivalentes a `develop`, pero aún no tiene una aprobación registrada en GitHub. El PR release-please #78 prepara v0.14.0; debe incluir las correcciones antes de publicar.

## 2. Inventario de hallazgos Dependabot

| Alerta | Severidad | Advisory | Riesgo resumido | Versión fijada |
|---|---|---|---|---|
| #16 | Media | GHSA-rq7h-c2jc-7f22 | Posible lectura de archivos fuera de `server.fs.allow` con rutas que coinciden con paths del sistema | Vite 8.3.3+ |
| #15 | Baja | GHSA-9jrq-w75r-8gcw | Ejecución de script cross-origin bajo ciertas configuraciones de middleware | Vite 8.3.3+ |
| #14 | Media | GHSA-vfpm-58rq-9qcg | Exposición de los primeros bytes de archivos fuera de `server.fs` mediante `vite-wasm-instance` | Vite 8.3.3+ |
| #13 | Alta | GHSA-587h-7wmj-h5pm | Bypass de comprobación de ruta de `launch-editor` en Windows | Vite 8.3.3+ |
| #12 | Media | GHSA-r825-w8mm-5pxw | Interpretación de argumentos por algunos editores que puede conducir a ejecución de comandos | Vite 8.3.3+ |
| #11 | Media | GHSA-698v-4r3x-3vcv | Escape incompleto de `%` en Windows en ciertos escenarios de `launch-editor` | Vite 8.3.3+ |

Las seis alertas apuntan al manifiesto `packages/synapsis-client/package-lock.json`; ninguna está marcada como dependencia de runtime. El nivel alto está condicionado a Windows y a que un atacante controle el argumento/ruta descrito por el advisory.

## 3. RCTF

En este plan, **RCTF** significa **Riesgo, Controles, Trazabilidad y Fallback**.

### R — Riesgo

- Mantener Vite `8.3.2` conserva seis advisories conocidas en la herramienta de desarrollo.
- Mantener versiones anteriores de Tokio, rustls, rustls-webpki, las raíces WebPKI y crypto-bigint conserva advisories RustSec, incluidos riesgos de seguridad de I/O, certificados TLS y operaciones criptográficas de tiempo constante.
- `crypto-bigint 0.7.6` está yanked pese a ser la versión corregida indicada por RustSec; actualizarla limpia vulnerabilidades pero conserva una advertencia que requiere decisión de mantenimiento.
- Publicar desde una rama de release creada antes de integrar estas correcciones dejaría el tag sin ellas, aunque los PR de seguridad sigan abiertos o pasen pruebas individualmente.
- Los PR de Dependabot no reciben secretos privados de Actions; el PR #83 ya falló al clonar Arca y el escaneo completo encontró avisos Quinn presentes en su base. Estos fallos no se tratarán como PASS ni se resolverán ampliando permisos.

### C — Controles

- Fijar el árbol reproducible en `package-lock.json` a Vite `8.3.5`; confirmar el paquete resuelto y el árbol npm, no solo el rango de `package.json`.
- Integrar la actualización Quinn y la exploración OSV sin agregar excepciones o ignores para los advisories.
- Mantener el token de repositorios privados limitado a los jobs que lo requieren. No publicar Arca, no mover secretos al PR ni usar `pull_request_target` para habilitar secretos a Dependabot.
- Ejecutar revisión independiente Oura y verificar cada hallazgo contra el lockfile, advisories oficiales, salida de npm/Cargo y checks de CI. La auditoría local terminó con cero vulnerabilidades y una advertencia yanked; el CI debe confirmar con su configuración de cargo-deny.
- El release debe apuntar a un SHA que ya contenga todas las correcciones aceptadas, changelog/manifest calculados desde ese estado y escaneo final limpio.

### T — Trazabilidad y pruebas

- Registrar alerta/advisory, manifiesto, rango afectado, versión corregida y evidencia de cierre.
- Validar el SDK: `npm ci`, `npm audit --audit-level=low`, `npm run build` y `npm test`.
- Validar Rust: `cargo audit`, `cargo deny check` (CI; cargo-deny no está instalado en este entorno), `cargo test --locked --features arca -- --test-threads=1`, `cargo check --locked --features arca` y `cargo fmt --check`.
- Validar el repositorio completo con OSV Scanner y los jobs de Linux, macOS, Windows, MSRV, seguridad, convenciones y workflows aplicables.
- Después de integrar, consultar de nuevo las alertas Dependabot y el release/tag de GitHub; un PR verde por sí solo no cierra el objetivo.

### F — Fallback y recuperación

- Si npm/Cargo cambia resolución fuera de lo esperado, restaurar únicamente el lockfile desde la rama de trabajo y rehacer la actualización de manera reproducible; no editar a mano hashes o integridades.
- Si una prueba falla por Arca inaccesible, conservar el error como bloqueo, corregir el acceso en la configuración de CI con el alcance mínimo o reejecutar en el contexto autorizado. No marcar la prueba como aprobada ni debilitar el control.
- Si aparecen nuevas advisories o incompatibilidades, detener el release, actualizar el inventario y escoger la versión mínima fijada compatible; no publicar un tag parcial.
- Si CI o el mantenedor rechaza la versión yanked `crypto-bigint 0.7.6`, detener el release y actualizar Arca/K-256 o su árbol criptográfico a una versión upstream aceptable; no ocultar el aviso en Cargo audit.
- Si el release-please #78 no contiene el SHA y las notas de seguridad, actualizar/regenerar el PR de release antes de publicarlo; conservar tags existentes y no reescribir historia.

## 4. Fases y gates

1. **Baseline:** confirmar repo, ramas, alerts, PRs, versiones y estado de CI; registrar SHAs y no tocar los checkouts con cambios preexistentes.
2. **Remediación:** incorporar el bump de Vite a `8.3.5`, la corrección Quinn de `main` y el escaneo OSV diario para `main`/`develop`; revisar que el lockfile no introduzca paquetes inesperados.
3. **Validación local e independiente:** ejecutar las validaciones de npm/Cargo indicadas, revisar el diff y solicitar Oura; confirmar cada salida de forma independiente. Cualquier advertencia yanked debe quedar explícita y ser aceptada por la revisión/CI antes de publicar.
4. **Integración:** publicar un PR de mantenimiento normal con el token privado disponible para el CI autorizado. Resolver o cerrar PRs duplicados solo después de comprobar que su cambio está incluido y sus alertas se resuelven.
5. **Rama de desarrollo:** revisar por separado el PR #82 a `develop`; incorporarlo solo con revisión de diff y CI válido, conservando la separación de ramas.
6. **Release:** actualizar el PR release-please #78 o el PR que lo sustituya para que el tag planificado contenga las correcciones. Ejecutar el gate final sobre el SHA exacto del release; publicar solo con todos los gates obligatorios verdes.

## 5. Resultados esperados

- Las seis alertas Dependabot se cierran automáticamente tras integrar el lockfile fijado a Vite `8.3.5`.
- Quinn/`quinn-proto` queda en versiones fijadas sin los advisories cubiertos; OSV vuelve a escanear diariamente y en PR/push/merge queue de `main` y `develop`.
- `npm audit` y `cargo audit` no reportan vulnerabilidades abiertas para los manifests/lockfiles del release; no se añaden ignores. El resultado local actual de Cargo es cero vulnerabilidades y una advertencia: `crypto-bigint 0.7.6` yanked.
- Build, pruebas y checks requeridos pasan; el resultado de Windows se informa explícitamente, considerando que el job actual está configurado como `continue-on-error`.
- Las tres pruebas del SDK verifican la serialización del request MCP, el envío de bearer token y el manejo de errores JSON-RPC.
- El changelog, manifest, versión, tag y artefactos del release corresponden al SHA que contiene las correcciones. La versión exacta la determina release-please después de reconciliar el PR #78; no se fija manualmente en este plan.
- No hay cambios funcionales intencionales al API/runtime del SDK ni al comportamiento del servicio: la actualización Vite es de desarrollo y Quinn es una actualización de dependencias con compatibilidad comprobada por la suite.

## 6. Resultados que no se admiten

- Cerrar/silenciar alertas, añadir excepciones a Cargo audit/OSV o afirmar remediación basándose solo en un PR abierto.
- Publicar un tag/release cuyo SHA no incluya los locks corregidos y las notas pertinentes.
- Publicar mientras cargo-deny/CI no haya validado el lockfile, o sin decisión documentada sobre `crypto-bigint 0.7.6` yanked.
- Interpretar los fallos del PR #83 por falta de secreto privado como pruebas aprobadas, o conceder secretos privados a un PR de Dependabot.
- Afirmar impacto en producción a partir de una dependencia marcada solo de desarrollo, o afirmar seguridad absoluta a partir de Oura/audit aislados.
- Perder, sobrescribir o reescribir trabajo local existente; forzar push, borrar tags, publicar Arca o modificar permisos/credenciales.
- Marcar como terminada la línea `develop` si #82 queda sin revisión o sin evidencia de integración.

## 7. Criterio de cierre

La tarea se cierra cuando el release publicado se identifica por versión, tag y SHA verificables; contiene las correcciones de Vite, Quinn y RustSec, los seis avisos Dependabot están cerrados, OSV/npm/Cargo no presentan vulnerabilidades abiertas relacionadas, cargo-deny y los gates obligatorios están verdes, se ha resuelto o aceptado formalmente la advertencia yanked de `crypto-bigint`, y la remediación de `develop` quedó integrada o registrada con evidencia y responsable como única excepción pendiente.

## 8. Estado de ejecución y evidencia local

- Rama `codex/synapsis-security-release-20261027`, basada en `origin/main` `25bab13`; tres commits cherry-picked de remediaciones previas (Quinn, Vite y OSV) y lockfile Cargo actualizado localmente.
- `packages/synapsis-client`: `npm ci`, `npm run build`, `npm test` (3 pruebas) y `npm audit --audit-level=low` pasaron; Vite resuelve a `8.3.5`, auditoría npm informa 0 vulnerabilidades.
- Rust: `cargo check --locked --features arca`, `cargo test --locked --features arca -- --test-threads=1` (suite completa; 1 ignorada), `cargo fmt --check` y `cargo audit` pasaron. Cargo audit informó 0 vulnerabilidades y una advertencia yanked para `crypto-bigint 0.7.6`.
- `cargo deny check` no se pudo ejecutar localmente: el subcomando `cargo-deny` no está instalado. El workflow CI sí lo ejecuta y debe pasar en la rama remota.
- Oura informó sin vulnerabilidades en el conjunto revisado y sin violaciones de código limpio; esto complementa, pero no reemplaza, Cargo audit/deny ni CI.
- Pendiente: escaneo OSV del SHA final, CI con acceso privado configurado para PR de mantenedor, revisión/merge, actualización del PR release-please, cierre de alerts y publicación/tag verificable. No hay release publicado por esta tarea aún.
