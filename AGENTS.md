# Synapsis Ecosystem — SecDevOps & Workflow Framework

## 1. SecDevOps (Desarrollo Seguro + Operaciones)

### Principios
- **Shift Left**: Seguridad desde el primer commit, no al final
- **Zero Trust**: Verificación continua, mínimo privilegio, microsegmentación
- **Defense in Depth**: Múltiples capas de seguridad sin single point of failure
- **Immutable Audit**: Toda operación queda registrada sin posibilidad de alteración

### Security Gates (CI/CD)

| Gate | Herramienta | Falla | Obligatorio | Job en CI |
|------|------------|-------|-------------|-----------|
| Formato | `cargo fmt` | Sí | Sí | `fmt` |
| Linting | `cargo clippy` | Sí | Sí | `clippy` |
| MSRV | `cargo check` (1.95.0) | Sí | Sí | `msrv` |
| Tests (Linux/macOS) | `cargo test` | Sí | Sí | `test` |
| Tests (Windows) | `cargo check` | No (continue-on-error) | No | `test-windows` |
| Workflow lint | `actionlint` | No (warning) | Sí | `actionlint` |
| Audit | `cargo audit` | Sí | No (continue-on-error) | `security` |
| Licencias | `cargo deny` | Sí | No | `deny` |
| Secrets | `gitleaks` | Sí | Sí | `secrets` |
| Unsafe | `cargo geiger` | No | No | `geiger` |
| OSV | `osv-scanner` | Sí | No | `OSV-Scanner` |

### Branch Strategy

```
main ──► release tags (vX.Y.Z)
  │
  └── develop ──► feature branches
        │
        ├── feat/*
        ├── fix/*
        ├── refactor/*
        ├── deps/*
        ├── docs/*
        └── ci/*
```

- `main`: Solo releases y merges desde `develop`
- `develop`: Integración, CI debe pasar
- `feat/*`, `fix/*`: Branches desde `develop`

### PR Lifecycle

```
1. Push branch → CI triggers (test, fmt, clippy, msrv, audit, gitleaks, codeql)
2. Open PR → Labeler adds type labels + PR Review validates title/conventions
3. Auto-approve si:
   - Todos CI checks pasan
   - < 500 líneas añadidas, < 20 archivos
   - Sin label `breaking` o `blocked`
   - DB PRs tienen schema version bump
4. Merge (squash) a develop
5. Release-please crea PR de release a main
6. Merge release PR → tag vX.Y.Z → CI build + deploy
```

---

## 2. SMART Goals System

Cada issue/tarea debe cumplir SMART:

| Criterio | Descripción | Ejemplo |
|----------|------------|---------|
| **S**pecific | Qué exactamente, no generalidades | "Añadir tool `mem_export` que exporte observaciones a JSON" |
| **M**easurable | Cómo medir éxito | "51 tests pasando, latencia < 2ms, cobertura > 80%" |
| **A**chievable | Realizable con recursos actuales | "Usar serde_json existente, no requiere nueva DB" |
| **R**elevant | Alinea con objetivos del proyecto | "Necesario para Fase 3 (Integración)" |
| **T**ime-bound | Deadline o milestone | "Para v0.12.0 (julio 2026)" |

### Issue Template (SMART)

```markdown
## Descripción
[Specific: qué y por qué]

## Criterios de Aceptación (Measurable)
- [ ] Criterio 1
- [ ] Criterio 2

## Limitaciones (Achievable)
- Scope actual: ...
- Fuera de scope: ...

## Alineación (Relevant)
- [ ] Fase del roadmap: ...
- [ ] Epic: ...

## Timeline (Time-bound)
- Target: vX.Y.Z / YYYY-MM-DD
- Dependencias: ...
```

---

## 3. ProductManager Priority System — MoSCoW+RICE

### Niveles de Prioridad

| Nivel | Tag | MoSCoW | RICE | Acción |
|-------|-----|--------|------|--------|
| P0 | `priority:critical` | Must have | Reach >500, Impact >3 | Siguiente sprint, no negociable |
| P1 | `priority:high` | Should have | Reach 100-500, Impact 2-3 | Este sprint si hay capacidad |
| P2 | `priority:medium` | Could have | Reach 10-100, Impact 1-2 | Backlog, próximo sprint |
| P3 | `priority:low` | Won't have (now) | Reach <10, Impact <1 | Backlog, requiere re-evaluación |

### RICE Scoring

```
RICE Score = (Reach × Impact × Confidence) / Effort

Reach:   # usuarios/agentes afectados por release
         1 = <10, 10 = 10-100, 100 = 100-1000, 500 = >1000

Impact:  Mejora percibida por usuario
         1 = mínima, 2 = media, 3 = alta, 4 = transformacional

Confidence: Qué tan seguros estamos de Reach e Impact
         0.5 = especulación, 0.8 = estimación con datos, 1.0 = datos concretos

Effort:  Días-hombre estimados
         1 = horas, 3 = días, 10 = semanas, 40 = meses
```

### Labels de Prioridad

- `priority:critical` (P0)
- `priority:high` (P1)
- `priority:medium` (P2)
- `priority:low` (P3)

### Labels de Tipo (para MoSCoW)

- `type:bug` — Corrección de error (Must have por defecto)
- `type:feature` — Nueva funcionalidad (Priorizar con RICE)
- `type:security` — Vulnerabilidad (P0 automático)
- `type:refactor` — Mejora interna (P2-P3)
- `type:tech-debt` — Deuda técnica (P2-P3)
- `type:dependency` — Actualización de dependencias (P1 automático)

### Sprints

- Duración: **2 semanas**
- Ceremonia: Planning (lunes) → Review (viernes semana 2)
- Capacidad: estimar en días-hombre por sprint
- WIP limit: 3 items por persona

---

## 4. Escalabilidad

### Principios

- **Stateless donde se pueda**: El core no guarda estado entre requests
- **Stateful controlado**: SQLite WAL mode con connection pooling
- **Multi-agente nativo**: Locks atómicos, sesiones únicas por agente
- **Zero-copy donde aplique**: Streaming, chunks, eventos SSE

### Arquitectura de Escalado

```
                    ┌──────────────┐
                    │   Cliente 1  │
                    │  (Agente IA) │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │   Synapsis   │
                    │   Server     │
                    │  (HTTP/SSE)  │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
       ┌──────▼─────┐ ┌───▼────┐ ┌───▼──────┐
       │  SQLite    │ │ File   │ │ Network  │
       │  + FTS5    │ │ Store  │ │ Transport│
       │ (WAL mode) │ │(Atomic)│ │ (QUIC)   │
       └────────────┘ └────────┘ └──────────┘
```

### Métricas de Escalado (Targets v1.0)

| Métrica | Actual | Target |
|---------|--------|--------|
| Observaciones/segundo | ~5000 | >10000 |
| Latencia búsqueda FTS5 | <1ms | <0.5ms |
| Sesiones concurrentes | 50 | 200+ |
| Tamaño DB | Ilimitado (WAL) | Ilimitado |
| Agentes simultáneos | 10+ | 50+ |
| Tiempo cold start | <20ms | <10ms |

---

## 5. Release Flow

```
develop ──► PR a main ──► tag vX.Y.Z ──► GitHub Release
                                      │
                                      ├── Linux (x86_64, aarch64)
                                      ├── macOS (x86_64, aarch64)
                                      └── Windows (x86_64)
```

- **Versionado**: Semver estricto (`vMAJOR.MINOR.PATCH`)
- **Changelog**: Generado automáticamente por release-please
- **Release notes**: Incluir breaking changes, new features, fixes, security

---

## 6. Ecosistema de Repos

| Repo | Descripción | Rama principal | Estado |
|------|------------|----------------|--------|
| `MethodWhite/synapsis` | Motor de memoria MCP | `develop` | Activo |
| `MethodWhite/synapsis-core` | Librería core (dominio, storage, PQC) | `main` | Activo |
| `MethodWhite/Arca` | Wallet autogestionado (privado) | `main` | Activo |
| `MethodWhite/synapsis-landing` | Landing page | `main` | Activo |

### Dependencias entre repos

```
synapsis ──► synapsis-core (público, git dependency)
    │
    └──► Arca (privado, optional feature --features arca)
```

- CI en `synapsis` clona `synapsis-core` como sibling con `[patch]`
- `Arca` no se clona en CI (privado, feature optional)
- Para builds locales con Arca: `cargo build --features arca`
