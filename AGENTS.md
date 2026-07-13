## Git Flow

This project follows strict gitflow. See [GIT_FLOW.md](./GIT_FLOW.md) for:
- Branch structure (main, development, feature/*, hotfix/*)
- Conventional commits with gitmoji
- How to create features, hotfixes, and releases

## Project Structure

```
populatrs/
├── backend/                # Único crate Rust
│   ├── Cargo.toml          # Dependencias unificadas
│   ├── Cargo.lock
│   └── src/
│       ├── main.rs         # Servidor Axum + scheduler
│       ├── lib.rs          # Lógica compartida (run_feed_check)
│       ├── config.rs       # Config vía env vars
│       ├── db.rs           # SQLite (migraciones + CRUD)
│       ├── models.rs       # Tipos de datos
│       ├── feed.rs         # RSS / YouTube fetching
│       ├── publisher/      # 9 publishers (Telegram, X, Mastodon...)
│       ├── template.rs     # Template rendering (minijinja)
│       ├── auth.rs         # OIDC PocketID + JWT validator
│       ├── middleware.rs   # Auth middleware
│       ├── embed.rs        # Frontend embebido (rust-embed)
│       └── routes/         # API endpoints (feeds, publishers, schedule, status)
├── frontend/               # React + Vite + TypeScript
│   ├── package.json
│   ├── vite.config.ts
│   └── src/
│       ├── App.tsx         # Router principal
│       ├── theme.ts        # Tema oscuro Ant Design
│       ├── pages/          # Dashboard, Feeds, Publishers, etc.
│       ├── components/     # Layout, etc.
│       └── store/          # Auth state (sessionStorage)
├── Dockerfile              # Multi-stage build (frontend + backend)
├── populatrs.container.example  # Podman Quadlet
├── AGENTS.md
├── GIT_FLOW.md
└── README.md
```

## Desarrollo

### Backend

```bash
# Desde backend/
cd backend

# Modo desarrollo (sin OIDC — auth bypass automático)
HOST=0.0.0.0 PORT=8080 RUST_LOG=info cargo run

# Producción (con OIDC)
OIDC_ISSUER_URL=https://pocketid.example.com \
  OIDC_CLIENT_ID=populatrs \
  OIDC_CLIENT_SECRET=... \
  cargo run

# Tests
cargo test
cargo clippy
```

### Frontend

```bash
cd frontend
npm install
npm run dev   # Puerto 5173, proxy a backend en :8080
```

## Variables de entorno

| Variable | Descripción | Default |
|---|---|---|
| `HOST` | IP de escucha | `0.0.0.0` |
| `PORT` | Puerto | `8080` |
| `DATABASE_URL` | Ruta a SQLite | `./data/populatrs.db` |
| `DATA_DIR` | Directorio de datos | `./data` |
| `CHECK_INTERVAL` | Minutos entre check de feeds | `60` |
| `TIMEZONE` | Zona horaria | `UTC` |
| `RUST_LOG` | Nivel de log | `info` |
| `LOG_FORMAT` | Formato de log | `pretty` |
| `OIDC_ISSUER_URL` | URL del issuer OIDC | — |
| `OIDC_CLIENT_ID` | Client ID OIDC | — |
| `OIDC_CLIENT_SECRET` | Client secret OIDC | — |
| `OIDC_REDIRECT_URI` | Callback URL | `http://localhost:8080/auth/callback` |

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

## Docker

```bash
docker build -t populatrs:latest .
docker run -p 8080:8080 \
  -v $(pwd)/data:/app/data \
  -e DATABASE_URL=/app/data/populatrs.db \
  populatrs:latest
```