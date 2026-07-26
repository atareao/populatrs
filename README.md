<div align="center">
  <h1>🚀 Populatrs</h1>
  <p><em>Publicador automatizado de feeds RSS con interfaz web de gestión</em></p>

[![CI](https://img.shields.io/github/actions/workflow/status/atareao/populatrs/ci.yml?branch=main&style=flat-square&logo=github&label=CI)](https://github.com/atareao/populatrs/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/atareao/populatrs?style=flat-square&logo=semver&sort=semver)](https://github.com/atareao/populatrs/releases)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square&logo=rust)](https://rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Docker](https://img.shields.io/badge/docker-ghcr.io-2496ED?style=flat-square&logo=docker)](https://github.com/atareao/populatrs/pkgs/container/populatrs)

</div>

Populatrs es un servidor web que monitoriza feeds RSS, Atom y canales de YouTube, y publica automáticamente las nuevas entradas en múltiples plataformas sociales. Incluye un panel de administración web, planificación cron, plantillas personalizadas por feed/publicador, y un histórico completo de publicaciones.

---

## ✨ Características

### 📡 Fuentes
- RSS 2.0 y Atom
- YouTube API v3 (canales, listas de reproducción)
- Resolución automática de URLs de YouTube a channel ID
- Cache condicional (ETag, Last-Modified)
- Deduplicación de contenido por GUID

### 🎯 10 Publicadores

| Plataforma | Autenticación |
|---|---|
| **Telegram** | Bot Token |
| **X (Twitter)** | OAuth 2.0 PKCE |
| **Mastodon** | Access Token |
| **LinkedIn** | OAuth 2.0 |
| **Matrix** | Access Token (HTML, salas) |
| **Bluesky** | App Password |
| **Threads** | OAuth 2.0 (Meta) |
| **Discord** | Webhook |
| **OpenObserve** | API Key |
| **Webhook** | URL personalizada |

### ⏰ Planificación
- Expresión cron con zona horaria configurable
- Ejecución en el mismo proceso del servidor web
- Disparo manual por feed desde la interfaz
- Dashboard con estadísticas de tiempos

### 🎨 Plantillas
- Motor [minijinja](https://github.com/mitsuhiko/minijinja) (compatible con Jinja2)
- Variables: `title`, `description`, `url`, `published`, `feed_id`
- Filtros: `truncate`, `word_limit`, `strip_html`
- Plantillas personalizadas por feed y por publicador

### 🔐 Autenticación
- OIDC vía PocketID en producción
- Bypass automático en desarrollo (login de prueba)
- OAuth 2.0 embebido para LinkedIn, Threads, X y Mastodon desde la interfaz web

---

## 🚀 Inicio rápido (desarrollo)

### Prerrequisitos

- **Rust 1.85+** — [rustup.rs](https://rustup.rs/)
- **pnpm** — `npm install -g pnpm`
- **Systemd**: `build-essential pkg-config libssl-dev` (Ubuntu/Debian) o `openssl pkg-config` (macOS)

### Backend

```bash
cd backend
HOST=0.0.0.0 PORT=3044 RUST_LOG=info cargo run
```

Sin variables OIDC el servidor activa el modo desarrollo con bypass de autenticación. Accede a `http://localhost:3044/auth/dev-login?email=dev@test.com` para obtener un token.

### Frontend

```bash
cd frontend
pnpm install
pnpm dev
```

El frontend arranca en `http://localhost:5173` con proxy al backend en `:3044`.

---

## ⚙️ Configuración

### Variables de entorno

| Variable | Descripción | Por defecto |
|---|---|---|
| `HOST` | IP de escucha | `0.0.0.0` |
| `PORT` | Puerto | `3044` |
| `DATABASE_URL` | Ruta a SQLite | `./data/populatrs.db` |
| `DATA_DIR` | Directorio de datos | `./data` |
| `TIMEZONE` | Zona horaria | `UTC` |
| `RUST_LOG` | Nivel de log | `info` |
| `LOG_FORMAT` | Formato de log (`pretty` o `json`) | `pretty` |
| `OIDC_ISSUER_URL` | URL del issuer OIDC | — |
| `OIDC_CLIENT_ID` | Client ID OIDC | — |
| `OIDC_CLIENT_SECRET` | Client secret OIDC | — |
| `OIDC_REDIRECT_URI` | URL de callback OIDC | `http://localhost:3044/auth/callback` |

### Modos de autenticación

| Modo | Variables OIDC | Comportamiento |
|---|---|---|
| **Desarrollo** | No definidas | Login de prueba en `/auth/dev-login?email=...` |
| **Producción** | Configuradas | Validación de JWTs contra PocketID |

---

## 📡 API REST

### Públicas

| Ruta | Método | Descripción |
|---|---|---|
| `/health` | GET | Health check |
| `/auth/login` | GET | Redirige al issuer OIDC |
| `/auth/callback` | GET | Callback OIDC |
| `/auth/dev-login` | GET | Login de prueba (solo desarrollo) |

### Protegidas (requieren JWT)

| Ruta | Método | Descripción |
|---|---|---|
| `/api/me` | GET | Perfil del usuario autenticado |
| `/api/feeds` | GET | Listar feeds |
| `/api/feeds` | POST | Crear feed |
| `/api/feeds/{id}` | PUT | Actualizar feed |
| `/api/feeds/{id}` | DELETE | Eliminar feed |
| `/api/feeds/{id}` | PATCH | Activar/desactivar feed |
| `/api/feeds/{id}/run` | POST | Ejecutar comprobación del feed |
| `/api/feeds/{id}/publish` | POST | Forzar publicación de posts pendientes |
| `/api/feeds/resolve-youtube` | POST | Resolver URL de YouTube a channel ID |
| `/api/publishers` | GET | Listar publicadores |
| `/api/publishers` | POST | Crear publicador |
| `/api/publishers/{id}` | PUT | Actualizar publicador |
| `/api/publishers/{id}` | DELETE | Eliminar publicador |
| `/api/publishers/{id}` | PATCH | Activar/desactivar publicador |
| `/api/publishers/{id}/test` | POST | Probar publicador |
| `/api/publishers/{id}/oauth/authorize` | GET | Iniciar flujo OAuth |
| `/api/publishers/{id}/oauth/callback` | POST | Completar flujo OAuth |
| `/api/schedule` | GET | Obtener planificación |
| `/api/schedule` | PUT | Actualizar planificación |
| `/api/storage` | GET | Configuración de almacenamiento |
| `/api/storage` | PUT | Actualizar almacenamiento |
| `/api/youtube` | GET | Clave API de YouTube |
| `/api/youtube` | PUT | Actualizar clave API de YouTube |
| `/api/status` | GET | Estadísticas del dashboard |
| `/api/logs/stream` | GET | Streaming de logs (SSE) |
| `/api/logs/history` | GET | Histórico de logs |
| `/api/logs/retention` | GET | Días de retención de logs |
| `/api/logs/retention` | PUT | Actualizar retención de logs |

---

## 🐳 Docker

### Construcción

```bash
docker build -t populatrs:latest .
```

### Ejecución

```bash
docker run -p 3044:3044 \
  -v $(pwd)/data:/app/data \
  -e DATABASE_URL=/app/data/populatrs.db \
  populatrs:latest
```

El Dockerfile usa compilación multi-etapa (Rust → Node → Alpine). La imagen expone el puerto `3044` y ejecuta el proceso como usuario `app` (UID 1000). Los datos persistentes se almacenan en `/app/data`.

### Podman Quadlet

```bash
cp populatrs.container ~/.config/containers/systemd/
cp populatrs.env ~/.config/containers/systemd/
systemctl --user daemon-reload
systemctl --user start populatrs
systemctl --user status populatrs
```

El archivo `.container` incluye health check en `/health`, reinicio automático y mapeo de puertos `3044:3044`.

---

## 🛠️ Desarrollo

### Tests

```bash
cd backend && cargo test
cd frontend && pnpm test
```

### Lint

```bash
cd backend && cargo clippy -- -D warnings && cargo fmt --all -- --check
```

### Frontend

```bash
cd frontend && pnpm build   # Genera dist/ para producción
```

El frontend compilado se sirve desde `./dist/` mediante `tokio::fs::read` — no se embeve en el binario.

---

## 🏗️ Arquitectura

```
                    ┌─────────────┐
                    │  Scheduler  │ (cron, mismo proceso)
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              │    Feed Manager         │
              │ (RSS/Atom/YouTube API)  │
              └────────────┬────────────┘
                           │
              ┌────────────┴────────────┐
              │   Publisher Manager     │
              │  (10 publicadores)      │
              └────────────┬────────────┘
                           │
              ┌────────────┴────────────┐
              │      SQLite (rusqlite)  │
              │  feeds · publishers     │
              │  published_posts · logs │
              │  feed_cache · settings  │
              └─────────────────────────┘
```

### Flujo de publicación

1. El **scheduler** (cron) o una petición manual disparan la comprobación de feeds
2. **Feed Manager** obtiene las entradas nuevas (RSS/Atom/YouTube), aplicando cache condicional
3. Se filtran los posts ya publicados consultando `published_posts` en SQLite
4. **Publisher Manager** renderiza la plantilla (minijinja) y envía a cada publicador configurado
5. Los resultados individuales se registran en `publish_results`
6. El post se marca como publicado en `published_posts`

### Base de datos (SQLite)

| Tabla | Propósito |
|---|---|
| `feeds` | Configuración de feeds |
| `publishers` | Credenciales serializadas como JSON |
| `feed_publishers` | Relación N:M entre feeds y publicadores |
| `published_posts` | Histórico de posts publicados |
| `publish_results` | Resultados individuales por publicador |
| `feed_cache` | ETags y Last-Modified |
| `settings` | Configuración general (schedule, etc.) |

---

## 📝 Licencia

MIT. Ver [LICENSE](LICENSE).