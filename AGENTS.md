# AGENT DIRECTIVES: OPENSPEC (SDD) + TDD WORKFLOW

## I. CORE PRINCIPLES & GOALS

- **Phase 0 — Legacy Support:** If modifying existing code without specs or tests, establish a baseline spec and characterization tests before introducing changes.
- **Phase 1 — SDD (OpenSpec):** No new code or tests may be written before a spec change proposal exists in `openspec/changes/<feature>/` and is approved by the user.
- **Phase 2 — TDD (Red-Green-Refactor):** Once the spec is approved, code MUST be developed strictly test-first using terminal commands.
- **Strict Verification:** Always run CLI test suites using terminal tools. Never assume code or tests pass/fail without CLI confirmation.

> **🚫 NO-SKIP CLAUSE**: Ninguna instrucción del usuario — incluyendo "adelante", "ejecuta", "procede", "go ahead", "sounds good", "looks good" o cualquier otra variante — invalida los pasos SDD → TDD. El agente debe completar SDD (generar change proposal + esperar aprobación explícita del spec) y TDD (RED → GREEN → REFACTOR) antes de escribir código de aplicación. Si el usuario da una orden ambigua, el agente DEBE responder: *"¿Quieres que genere el change proposal en openspec/changes/ primero?"* antes de implementar.
> 
> **Consecuencia**: Cualquier código escrito sin seguir SDD + TDD se considera una violación del proceso. El agente debe detenerse, crear el spec retroactivo, y rehacer el trabajo con TDD.

> **🔁 FOLLOW-UP CLAUSE**: Cada ronda de ajustes, correcciones o refinamientos sobre un cambio ya archivado requiere SU PROPIO change proposal (ej: `ui-fixes-v2`, `auth-refactor-round-2`). Un cambio archivado NO autoriza modificaciones directas al código. El agente DEBE crear un nuevo change proposal incremental antes de tocar cualquier archivo fuente.

> **📁 SPEC FILE NAMING**: Los archivos spec delta dentro de `openspec/changes/<feature>/specs/<capability>/` DEBEN llamarse `spec.md`. El nombre `layout.md`, `auth.md`, etc. NO es válido — el validador de `openspec archive` los ignora. La estructura correcta es: `specs/ui/layout/spec.md` (no `specs/ui/layout.md`).

> **✅ APPROVED MARKER**: Tras la aprobación explícita del usuario, el agente DEBE crear un archivo `.approved` vacío en el directorio del change proposal (`touch openspec/changes/<feature>/.approved`). Esto evita que `require-proposal.sh` siga mostrando el warning "sin aprobación explícita" en ejecuciones posteriores.

> **🧹 POST-ARCHIVE CLEANUP**: Después de ejecutar `openspec archive <feature> --yes`, el agente DEBE verificar que el directorio `openspec/changes/<feature>/` fue eliminado. Si el archive no lo limpia automáticamente, el agente DEBE eliminarlo manualmente (`rm -rf openspec/changes/<feature>/`). De lo contrario, `require-proposal.sh` lo detectará como proposal activo fantasma.

> **⚠️ ARCHIVE PRE-FLIGHT**: Antes de archivar, verificar que la ruta destino en `openspec/specs/` no tenga ya un spec que entre en conflicto. Si existe un spec en `specs/ui/layout/spec.md` y el archive va a crear `specs/ui/spec.md`, hay que mover/renombrar primero para evitar duplicación.

> **⚠️ PRE-FLIGHT OBLIGATORIO**: Antes de CUALQUIER tool call que lea o escriba archivos de código (`backend/src/*.rs`, `frontend/src/*.{tsx,ts}`), el agente DEBE ejecutar `source openspec/require-proposal.sh`. Si falla, el agente DEBE detenerse y crear un change proposal primero. Esta validación es innegociable.

---

## II. EXECUTION WORKFLOW

### Phase 0: Legacy Code Preparation (Conditional)

*Execute this phase ONLY if modifying an existing module/file that lacks OpenSpec documentation or tests.*

1. **Characterization Spec (As-Is):**
   - Inspect the target file/module.
   - Generate a baseline spec in `openspec/specs/<module>/spec.md` reflecting current behavior.
2. **Characterization Tests:**
   - Write Rust (`#[test]`) or React/TS (`vitest` / `@testing-library/react`) tests matching current behavior.
   - Run tests via CLI (`cargo test` or `npx vitest run`) to confirm all pass in **GREEN**.

### Phase 1: SDD Protocol (OpenSpec)

When the user requests a new feature, bug fix, or refactor:

0. **🔒 PRE-FLIGHT:**
   - Run `source openspec/validate-workflow.sh` as the **first step** before anything else.
   - If it fails, STOP. Create the change proposal first.

1. **Create the Change Proposal:****
   - Execute CLI command: `openspec new change <feature-name>`
2. **Draft Specifications:**
   - Populate `openspec/changes/<feature-name>/proposal.md` with intent, scope, and impact.
   - Create spec deltas in `openspec/changes/<feature-name>/specs/<module>/spec.md`.
   - Ensure the spec includes:
     - **Contracts:** Rust types/structs/enums, TypeScript interfaces/props, API endpoints, or function signatures.
     - **Scenarios (BDD style):** Detailed `Given / When / Then` clauses for happy path, error cases, and edge cases.
   - Populate `openspec/changes/<feature-name>/tasks.md` with the TDD task checklist.
3. **STOP & WAIT FOR APPROVAL:**
   - Present the created specification to the user.
   - **DO NOT** write application code or new tests until the user explicitly approves the spec.
   - **Tras aprobación explícita**: crear el archivo `.approved`:
     ```bash
     touch openspec/changes/<feature-name>/.approved
     ```

### Phase 2: TDD Protocol (Red-Green-Refactor)

Once the user approves the spec (e.g., "Approved", "Looks good", "Proceed with TDD"):

1. **RED (Write Failing Tests):**
   - Read the `Given / When / Then` scenarios in `openspec/changes/<feature-name>/specs/`.
   - Write tests in Rust or React/TypeScript corresponding to those scenarios.
   - Execute CLI tests (`cargo test` or `npx vitest run`).
   - **Verify:** Confirm test failure for the new functionality while any legacy tests remain **GREEN**.
2. **GREEN (Minimal Implementation):**
   - Write the absolute minimum code necessary to satisfy the failing tests.
   - Execute CLI tests (`cargo test` or `npx vitest run`).
   - Run type checks (`cargo check` or `npx tsc -b`).
   - **Verify:** Confirm all tests pass (100% green) and no compilation/type errors exist.
3. **REFACTOR (Clean & Consolidate):**
   - Clean up code formatting, types, and structure without altering behavior.
   - Run linters (`cargo clippy -- -D warnings` / `npm run lint`).
   - Re-run test suites via CLI to guarantee no regressions.
4. **CONSOLIDATE & ARCHIVE:**
   - Mark completed items in `tasks.md`.
   - **PRE-FLIGHT**: Verificar que la ruta destino en `openspec/specs/` no tenga ya un spec que entre en conflicto (ej: `specs/ui/spec.md` vs `specs/ui/layout/spec.md`).
   - Once all scenarios pass, run `openspec archive <feature-name> --yes` to merge the delta into `openspec/specs/`.
   - **POST-ARCHIVE CLEANUP**: Verificar que `openspec/changes/<feature-name>/` fue eliminado. Si no, eliminarlo manualmente:
     ```bash
     rm -rf openspec/changes/<feature-name>/
     ```

---

## III. PROJECT CONFIGURATION & CONVENTIONS

### Stack Commands

#### Backend: Rust
- **Test Runner:** `cargo test` (or `cargo nextest run` if available).
- **Type Checking & Linting:** `cargo check` and `cargo clippy -- -D warnings` (enforce zero warnings).
- **Formatting:** `cargo fmt --check`
- **Conventions:**
  - Structs and types placed in domain modules or `src/models/`.
  - Unit tests placed in the same file under `#[cfg(test)]`.
  - Integration and API tests placed in `tests/`.

#### Frontend: React + TypeScript
- **Test Runner:** `npx vitest run` or `npm test -- --watch=false` (single-pass execution).
- **Type Checking:** `npx tsc -b` (mandatory during GREEN/REFACTOR steps).
- **Linting & Formatting:** `npm run lint` / `npx eslint .`
- **Conventions:**
  - Components in `src/components/`, hooks in `src/hooks/`.
  - Component tests colocated as `Component.test.tsx` using `@testing-library/react`.
  - User-centric testing behavior using `@testing-library/user-event` instead of implementation details.

### Custom Repository Rules

- Insert here any specific business logic, database conventions, or custom architectural rules unique to this project.

---

## IV. RESPONSE FORMAT & STATUS MESSAGES

Always prefix your progress updates with the current status tag:

```text
[LEGACY - INSPECT] Creating baseline spec & characterization tests.
[OPENSPEC - DRAFT] Generating change proposal in openspec/changes/...
[OPENSPEC - WAITING] Spec generated. Awaiting user review and approval.
[TDD - RED] Creating tests for scenario <Name> -> Running CLI tests.
[TDD - GREEN] Implementing minimal code -> Running CLI tests & type checks.
[TDD - REFACTOR] Refactoring code -> Running Clippy/ESLint & tests.
[OPENSPEC - ARCHIVE] Archiving change into openspec/specs/.
```


---

## V. CURRENT PROJECT STATE


### Project Overview


#### Project Structure

```
populatrs/
├── backend/                # Único crate Rust
│   ├── Cargo.toml          # Dependencias (v0.2.0)
│   ├── Cargo.lock
│   ├── run.sh              # Script de desarrollo
│   └── src/
│       ├── main.rs         # Servidor Axum + scheduler
│       ├── lib.rs          # Lógica compartida (run_feed_check)
│       ├── config.rs       # Config vía env vars
│       ├── db.rs           # SQLite (migraciones + CRUD)
│       ├── models.rs       # Tipos de datos
│       ├── feed.rs         # RSS / YouTube fetching
│       ├── template.rs     # Template rendering (minijinja)
│       ├── auth.rs         # OIDC PocketID + JWT validator
│       ├── middleware.rs   # Auth middleware
│       ├── embed.rs        # Frontend servido desde disco (tokio::fs)
│       ├── publisher/      # 9 publishers (bluesky, discord, linkedin,
│       │                   #   mastodon, matrix, openobserve, telegram,
│       │                   #   threads, x)
│       └── routes/         # API endpoints
│           ├── mod.rs      # Router principal (públicas + protegidas)
│           ├── auth_routes.rs  # Login OIDC + dev-login
│           ├── feeds.rs    # CRUD + run de feeds
│           ├── publishers.rs   # List + update de publishers
│           ├── schedule.rs # GET/PUT schedule
│           ├── status.rs   # Dashboard status
│           └── storage.rs  # Config de almacenamiento
├── frontend/               # React + Vite + TypeScript
│   ├── package.json
│   ├── pnpm-lock.yaml
│   ├── pnpm-workspace.yaml
│   ├── .npmrc
│   ├── vite.config.ts
│   ├── index.html
│   ├── dist/               # Build de producción
│   └── src/
│       ├── main.tsx        # Entry point
│       ├── App.tsx         # Router principal
│       ├── theme.ts        # Tema oscuro Ant Design
│       ├── global.css      # Estilos globales
│       ├── api/            # Cliente HTTP (http.ts)
│       ├── components/     # Componentes (AppLayout.tsx)
│       ├── hooks/          # Hooks (useAuth.ts)
│       ├── pages/          # Dashboard, Feeds, Publishers, Schedule,
│       │                   #   Settings, LoginPage, OAuthCallback, LogsPage
│       ├── store/          # Auth state (auth.ts)
│       └── test/           # Tests (setup.ts + tests por página)
├── .justfile               # Task runner (just) — build, push, version
├── .vampus.yml             # Version management
├── Dockerfile              # Multi-stage build (Rust → Node → Alpine)
├── populatrs.container     # Podman Quadlet (producción)
├── populatrs.container.example  # Podman Quadlet de ejemplo
├── populatrs.env           # Env vars para Quadlet
├── AGENTS.md
├── GIT_FLOW.md
├── CHANGELOG.md
├── cliff.toml              # Changelog generation config
└── README.md
```

#### Desarrollo

##### Backend

```bash
# Desde backend/
cd backend

# Modo desarrollo (sin OIDC — auth bypass automático)
HOST=0.0.0.0 PORT=3044 RUST_LOG=info cargo run

# Producción (con OIDC)
OIDC_ISSUER_URL=https://pocketid.example.com \
  OIDC_CLIENT_ID=populatrs \
  OIDC_CLIENT_SECRET=... \
  cargo run

# Tests
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

### Frontend

```bash
cd frontend
pnpm install
pnpm dev          # Puerto 5173, proxy a backend en :3044
pnpm build        # Build producción → dist/
pnpm test         # Tests con Vitest
```

### Just (task runner)

```bash
just list     # Lista recetas disponibles
just build    # Build imagen Podman
just push     # Push a registry
just version  # Bump patch + tag
```

## Variables de entorno

| Variable | Descripción | Default |
|---|---|---|
| `HOST` | IP de escucha | `0.0.0.0` |
| `PORT` | Puerto | `3044` |
| `DATABASE_URL` | Ruta a SQLite | `./data/populatrs.db` |
| `DATA_DIR` | Directorio de datos | `./data` |
| `TIMEZONE` | Zona horaria | `UTC` |
| `RUST_LOG` | Nivel de log | `info` |
| `LOG_FORMAT` | Formato de log | `pretty` |
| `OIDC_ISSUER_URL` | URL del issuer OIDC | — |
| `OIDC_CLIENT_ID` | Client ID OIDC | — |
| `OIDC_CLIENT_SECRET` | Client secret OIDC | — |
| `OIDC_REDIRECT_URI` | Callback URL | `http://localhost:3044/auth/callback` |

#### Modos de auth

- **Producción**: Configurar `OIDC_ISSUER_URL` y `OIDC_CLIENT_ID`. El servidor valida JWTs contra PocketID.
- **Desarrollo**: Sin variables OIDC, el servidor usa un JWT validator de desarrollo que acepta cualquier token. Usar `/auth/dev-login?email=dev@test.com` para obtener un token de prueba.

#### Arquitectura

##### Base de datos (SQLite)

| Tabla | Propósito |
|---|---|
| `feeds` | Configuración de feeds RSS/YouTube |
| `publishers` | Configuración de publishers (credenciales serializadas como JSON) |
| `feed_publishers` | Relación N:M entre feeds y publishers |
| `published_posts` | Histórico de posts publicados |
| `publish_results` | Resultados individuales por publisher |
| `feed_cache` | ETags / Last-Modified para conditional requests |
| `settings` | Configuración general (schedule, etc.) |

### Scheduler

El scheduler se ejecuta en un `tokio::spawn` dentro del mismo proceso del servidor web.
Cada ciclo recarga feeds y publishers desde SQLite, ejecuta el feed check, y espera
el intervalo configurado.

##### Pipeline de publicación

1. Fetch feed (RSS/Atom/YouTube API) → lista de posts
2. Filtrar posts ya publicados (check en SQLite por guid+feed_id)
3. Para cada post nuevo: renderizar template y publicar en todos los publishers asignados
4. Registrar resultados en `publish_results`
5. Marcar post como publicado en `published_posts`

#### Despliegue

##### Docker

```bash
docker build -t populatrs:latest .
docker run -p 3044:3044 \
  -v $(pwd)/data:/app/data \
  -e DATABASE_URL=/app/data/populatrs.db \
  populatrs:latest
```

#### Notas técnicas

- **Frontend estático**: Ya no se embebe con `rust-embed`. El servidor sirve archivos desde `./dist/` con `tokio::fs::read`.
- **pnpm**: El frontend usa pnpm workspaces, no npm.
- **Auth**: OIDC obligatorio en producción (PocketID). En desarrollo hay bypass automático.
- **Quadlet**: Único estándar de despliegue. No usar docker-compose ni podman-compose.
