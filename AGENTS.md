## Git Flow

This project follows strict gitflow. See [GIT_FLOW.md](./GIT_FLOW.md) for:
- Branch structure (main, development, feature/*, hotfix/*)
- Conventional commits with gitmoji
- How to create features, hotfixes, and releases

## Project Structure

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

## Desarrollo

### Backend

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
| `CHECK_INTERVAL` | Minutos entre check de feeds | `60` |
| `TIMEZONE` | Zona horaria | `UTC` |
| `RUST_LOG` | Nivel de log | `info` |
| `LOG_FORMAT` | Formato de log | `pretty` |
| `OIDC_ISSUER_URL` | URL del issuer OIDC | — |
| `OIDC_CLIENT_ID` | Client ID OIDC | — |
| `OIDC_CLIENT_SECRET` | Client secret OIDC | — |
| `OIDC_REDIRECT_URI` | Callback URL | `http://localhost:3044/auth/callback` |

## Modos de auth

- **Producción**: Configurar `OIDC_ISSUER_URL` y `OIDC_CLIENT_ID`. El servidor valida JWTs contra PocketID.
- **Desarrollo**: Sin variables OIDC, el servidor usa un JWT validator de desarrollo que acepta cualquier token. Usar `/auth/dev-login?email=dev@test.com` para obtener un token de prueba.

## Arquitectura

### Base de datos (SQLite)

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

### Pipeline de publicación

1. Fetch feed (RSS/Atom/YouTube API) → lista de posts
2. Filtrar posts ya publicados (check en SQLite por guid+feed_id)
3. Para cada post nuevo: renderizar template y publicar en todos los publishers asignados
4. Registrar resultados en `publish_results`
5. Marcar post como publicado en `published_posts`

## Despliegue

### Docker

```bash
docker build -t populatrs:latest .
docker run -p 3044:3044 \
  -v $(pwd)/data:/app/data \
  -e DATABASE_URL=/app/data/populatrs.db \
  populatrs:latest
```

### Podman Quadlet (producción)

```bash
# Copiar archivos a ~/.config/containers/systemd/
cp populatrs.container ~/.config/containers/systemd/
cp populatrs.env ~/.config/containers/systemd/

# Recargar y arrancar
systemctl --user daemon-reload
systemctl --user start populatrs

# Estado
systemctl --user status populatrs
```

## Notas técnicas

- **Frontend estático**: Ya no se embebe con `rust-embed`. El servidor sirve archivos desde `./dist/` con `tokio::fs::read`.
- **pnpm**: El frontend usa pnpm workspaces, no npm.
- **Auth**: OIDC obligatorio en producción (PocketID). En desarrollo hay bypass automático.
- **Quadlet**: Único estándar de despliegue. No usar docker-compose ni podman-compose.