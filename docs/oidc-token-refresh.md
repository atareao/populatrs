# OIDC Token Refresh: Guía de Implementación

> **Propósito**: Documentar el patrón de refresh token OIDC para aplicaciones self-hosted
> que usan PocketID como proveedor de identidad. Este patrón es aplicable a cualquier
> cliente OIDC (Populatrs, TinyAuth, o cualquier otra herramienta).
>
> **Versión**: 1.0
> **Fecha**: 2026-08-26

---

## Índice

1. [El problema](#1-el-problema)
2. [Arquitectura del flujo OIDC](#2-arquitectura-del-flujo-oidc)
3. [PocketID: configuración de tokens](#3-pocketid-configuración-de-tokens)
4. [Patrón de implementación](#4-patrón-de-implementación)
5. [Implementación en backend](#5-implementación-en-backend)
6. [Implementación en frontend](#6-implementación-en-frontend)
7. [Casos extremos](#7-casos-extremos)
8. [Checklist de implementación](#8-checklist-de-implementación)

---

## 1. El problema

### Síntomas

- Al abrir Populatrs en una pestaña nueva, redirige al login de PocketID
- Tras ~1 hora sin usar la app, cualquier interacción redirige al login
- Al cerrar y reabrir el navegador, la sesión se pierde siempre

### Causa raíz

El token JWT que emite PocketID tiene una validez limitada (por defecto 1 hora).
Populatrs actualmente:

1. **No guarda el token en `localStorage`** — solo en `sessionStorage`, que es
   específico de cada pestaña y se borra al cerrar el navegador
2. **No captura el `refresh_token`** — el struct `TokenResponse` ignora
   `expires_in` y no tiene campo para `refresh_token`
3. **No tiene endpoint `/auth/refresh`** — no hay forma de obtener un token
   nuevo sin hacer login completo en PocketID
4. **No intercepta 401s** — cuando el token expira, `fetchMe()` falla y se
   redirige al login sin intentar refrescar

---

## 2. Arquitectura del flujo OIDC

### Flujo actual (roto)

```
Usuario → Login en PocketID → Código de autorización
  → Backend canjea código por tokens en /api/oidc/token
  → Backend guarda SOLO id_token en sessionStorage
  → Frontend usa token en cada llamada API (Authorization: Bearer)
  → Token expira (1h) → Backend responde 401
  → Frontend borra token → Redirige al login ← ❌
```

### Flujo deseado (con refresh)

```
Usuario → Login en PocketID → Código de autorización
  → Backend canjea código por tokens en /api/oidc/token
  → Backend recibe: access_token + refresh_token + expires_in + id_token
  → Backend guarda access_token en localStorage
  → Backend guarda refresh_token en SQLite (asociado al usuario)
  → Frontend usa access_token en cada llamada API
  → Token expira (1h) → Backend responde 401
  → Frontend llama a POST /auth/refresh con refresh_token
  → Backend canjea refresh_token en PocketID → nuevos tokens
  → Backend guarda nuevo access_token + nuevo refresh_token (rotación)
  → Frontend reintenta la llamada original ← ✅
  → Si refresh_token también expira (30d) → Redirige al login
```

---

## 3. PocketID: configuración de tokens

PocketID usa la librería [`fosite`](https://github.com/ory/fosite) de Ory.
Los valores por defecto relevantes son:

| Parámetro | Valor | Configurable |
|---|---|---|
| Access token lifespan | 1 hora | No directamente |
| Refresh token lifespan | **30 días** | `RefreshTokenLifespan` en código |
| Rotación de refresh token | **Sí** | Siempre activa |
| Grant types soportados | `authorization_code`, `refresh_token`, `client_credentials`, `device_code` | Sí |

### Endpoints OIDC de PocketID

| Endpoint | Propósito |
|---|---|
| `{ISSUER}/.well-known/openid-configuration` | Descubrimiento OIDC |
| `{ISSUER}/.well-known/jwks.json` | Claves públicas JWKS |
| `{ISSUER}/authorize` | Autorización OIDC (login) |
| `{ISSUER}/api/oidc/token` | Canje de código + refresh token |
| `{ISSUER}/api/oidc/userinfo` | Información del usuario |

### Token endpoint: parámetros

**Para canje de código (`grant_type=authorization_code`)**:

```
POST {ISSUER}/api/oidc/token
Content-Type: application/x-www-form-urlencoded

grant_type=authorization_code
&code={code}
&redirect_uri={redirect_uri}
&client_id={client_id}
&client_secret={client_secret}
```

**Respuesta**:
```json
{
  "access_token": "eyJ...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "def502...",
  "id_token": "eyJ..."
}
```

**Para refresh (`grant_type=refresh_token`)**:

```
POST {ISSUER}/api/oidc/token
Content-Type: application/x-www-form-urlencoded

grant_type=refresh_token
&refresh_token={refresh_token}
&client_id={client_id}
&client_secret={client_secret}
```

**Respuesta** (con rotación):
```json
{
  "access_token": "eyJ...nuevo",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "def502...nuevo"
}
```

> ⚠️ **Rotación**: PocketID rota el refresh token en cada uso. El refresh token
> anterior se invalida inmediatamente. Esto significa que:
> - Cada refresh produce un NUEVO refresh_token
> - El refresh_token anterior deja de funcionar
> - Si dos refreshes concurrentes usan el mismo token, uno fallará

---

## 4. Patrón de implementación

### Decisión arquitectónica: ¿dónde guardar el refresh_token?

| Opción | Ventajas | Desventajas |
|---|---|---|
| **SQLite (backend)** | Persistente, seguro (httpOnly), compartido entre dispositivos | Requiere endpoint `/auth/refresh` |
| **localStorage (frontend)** | Simple de implementar | Vulnerable a XSS, no httpOnly |
| **Cookie httpOnly** | Seguro contra XSS | Complejo de manejar con SPA |

**Recomendación para self-hosted**: Guardar el refresh_token en SQLite en el
backend. Es la opción más segura y consistente. El frontend nunca toca el
refresh_token directamente.

### Estrategia de refresh

Hay dos estrategias complementarias:

1. **Refresh reactivo** (obligatorio): Cuando el backend responde 401, el
   frontend intenta refrescar antes de redirigir al login.
2. **Refresh proactivo** (opcional): El frontend calcula cuándo expira el token
   y lo refresca antes de que caduque, evitando el 401.

Para herramientas self-hosted, el **refresh reactivo** es suficiente. El
proactivo añade complejidad innecesaria.

---

## 5. Implementación en backend

### 5.1 Capturar refresh_token en el callback OIDC

**Archivo**: `backend/src/routes/auth_routes.rs`

El struct `TokenResponse` actual ignora `refresh_token` y `expires_in`:

```rust
// ❌ Actual: ignora refresh_token y expires_in
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    expires_in: u64,       // ← marcado como dead_code
    id_token: Option<String>,
    // ❌ Falta: refresh_token
}
```

**Cambio necesario**:

```rust
// ✅ Nuevo: captura todo
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
    id_token: Option<String>,
    refresh_token: Option<String>,  // ← NUEVO
}
```

### 5.2 Guardar refresh_token en SQLite

**Archivo**: `backend/src/db.rs`

Añadir una tabla o usar la tabla `settings` existente para almacenar el
refresh_token asociado al usuario:

```sql
-- Tabla para refresh tokens
CREATE TABLE IF NOT EXISTS refresh_tokens (
    user_id TEXT PRIMARY KEY,
    refresh_token TEXT NOT NULL,
    expires_at TEXT NOT NULL,       -- ISO 8601
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**Operaciones CRUD**:

```rust
// En Database
pub async fn save_refresh_token(&self, user_id: &str, token: &str, expires_in: u64) -> Result<()>
pub async fn get_refresh_token(&self, user_id: &str) -> Result<Option<String>>
pub async fn delete_refresh_token(&self, user_id: &str) -> Result<()>
```

### 5.3 Endpoint POST /auth/refresh

**Archivo**: `backend/src/routes/auth_routes.rs`

```rust
/// Refresca el access_token usando el refresh_token almacenado.
/// 
/// 1. Lee el refresh_token de SQLite para el usuario autenticado
/// 2. Lo canjea en PocketID por un nuevo access_token + refresh_token
/// 3. Guarda el nuevo refresh_token (rotación)
/// 4. Devuelve el nuevo access_token al frontend
pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<RefreshResponse>, StatusCode> {
    // 1. Obtener refresh_token de SQLite
    let refresh_token = state.db
        .get_refresh_token(&user.user_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 2. Canjear en PocketID
    let issuer = state.config.oidc_issuer_url.as_deref().ok_or(StatusCode::BAD_GATEWAY)?;
    let token_url = format!("{}/api/oidc/token", issuer.trim_end_matches('/'));

    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh_token),
        ("client_id", state.config.oidc_client_id.as_deref().unwrap_or("")),
        ("client_secret", state.config.oidc_client_secret.as_deref().unwrap_or("")),
    ];

    let client = reqwest::Client::new();
    let resp = client.post(&token_url).form(&params).send().await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !resp.status().is_success() {
        // Refresh token expirado o inválido → limpiar y forzar login
        state.db.delete_refresh_token(&user.user_id).await.ok();
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token_data: TokenResponse = resp.json().await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    // 3. Guardar nuevo refresh_token (rotación)
    if let Some(new_refresh) = &token_data.refresh_token {
        state.db.save_refresh_token(
            &user.user_id,
            new_refresh,
            token_data.expires_in,
        ).await.ok();
    }

    // 4. Devolver nuevo access_token
    Ok(Json(RefreshResponse {
        access_token: token_data.access_token,
        expires_in: token_data.expires_in,
    }))
}
```

### 5.4 Ruta pública para /auth/refresh

El endpoint `/auth/refresh` debe ser **público** (sin validación de token JWT,
porque el token puede estar expirado). En su lugar, se autentica mediante el
refresh_token almacenado en SQLite.

```rust
// En main.rs, función is_public_path:
pub(crate) fn is_public_path(path: &str) -> bool {
    // ...
    || path == "/auth/refresh"    // ← AÑADIR
    // ...
}
```

### 5.5 Seguridad del endpoint /auth/refresh

El endpoint debe estar protegido aunque sea público:

1. **Rate limiting**: Máximo N intentos por minuto por IP
2. **Validación de usuario**: El refresh_token debe corresponder al usuario
   que hace la petición (se identifica por el token JWT expirado o por
   el propio refresh_token)
3. **Limpieza en error**: Si el refresh falla, eliminar el token almacenado
   para evitar reintentos infinitos

---

## 6. Implementación en frontend

### 6.1 Guardar token en localStorage

**Archivo**: `frontend/src/store/auth.ts`

```typescript
// ✅ Guardar en ambos almacenes
export function setToken(token: string): void {
  try {
    sessionStorage.setItem("populatrs_token", token);
    localStorage.setItem("populatrs_token", token);
  } catch { /* noop */ }
}
```

### 6.2 Interceptor de 401 con refresh

**Archivo**: `frontend/src/api/http.ts`

```typescript
// Estado global para evitar múltiples refreshes concurrentes
let refreshPromise: Promise<string | null> | null = null;

async function refreshAccessToken(): Promise<string | null> {
  try {
    const res = await fetch("/auth/refresh", { method: "POST" });
    if (!res.ok) return null;
    const data = await res.json();
    const newToken = data.access_token;
    setToken(newToken);
    return newToken;
  } catch {
    return null;
  }
}

async function fetcher<T>(
  path: string,
  opts?: { method?: string; body?: unknown }
): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(path, {
    method: opts?.method ?? (opts?.body ? "POST" : "GET"),
    headers,
    body: opts?.body ? JSON.stringify(opts.body) : undefined,
  });

  // 🔄 Interceptor 401: intentar refresh
  if (res.status === 401 && token) {
    // Evitar múltiples refreshes concurrentes
    if (!refreshPromise) {
      refreshPromise = refreshAccessToken().finally(() => {
        refreshPromise = null;
      });
    }

    const newToken = await refreshPromise;
    if (newToken) {
      // Reintentar con el nuevo token
      headers["Authorization"] = `Bearer ${newToken}`;
      const retry = await fetch(path, {
        method: opts?.method ?? (opts?.body ? "POST" : "GET"),
        headers,
        body: opts?.body ? JSON.stringify(opts.body) : undefined,
      });
      if (retry.ok) {
        if (retry.status === 204) return undefined as T;
        return retry.json();
      }
    }

    // Refresh falló → limpiar y dejar que ProtectedRoute redirija
    clearToken();
    throw new Error("Session expired");
  }

  if (!res.ok) {
    const text = await res.text().catch(() => "unknown error");
    throw new Error(`HTTP ${res.status}: ${text}`);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}
```

### 6.3 Actualizar useAuth para manejar refresh

**Archivo**: `frontend/src/hooks/useAuth.ts`

```typescript
export function useAuth() {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const token = getToken();
    if (!token) {
      setLoading(false);
      return;
    }

    fetchMe()
      .then(setUser)
      .catch(() => {
        // NO limpiar token aquí — el interceptor de 401 ya lo hizo
        // si el refresh falló. Si el error es de red, el token sigue
        // siendo válido y no debemos borrarlo.
        setUser(null);
      })
      .finally(() => setLoading(false));
  }, []);

  return { user, loading };
}
```

---

## 7. Casos extremos

### 7.1 Refresco concurrente

Si dos peticiones API reciben 401 simultáneamente, solo debe ejecutarse un
refresh. Las demás deben esperar al mismo refresh y usar el nuevo token.

**Solución**: Variable `refreshPromise` compartida (ver sección 6.2).

### 7.2 Refresh token expirado (30 días)

Si el refresh_token ha expirado (>30 días sin usar la app), PocketID devuelve
error. El backend debe:

1. Eliminar el refresh_token de SQLite
2. Devolver 401 al frontend
3. El frontend redirige al login de PocketID

### 7.3 Rotación de refresh token

PocketID invalida el refresh_token anterior al emitir uno nuevo. Esto implica:

- **Race condition**: Si dos refreshes concurrentes usan el mismo token, el
  segundo recibirá error. El interceptor con `refreshPromise` lo resuelve.
- **Pérdida de token**: Si el backend guarda el nuevo token pero la respuesta
  no llega al frontend, el token queda inconsistente. Solución: el frontend
  debe confirmar la recepción, o el backend debe mantener el token anterior
  como respaldo.

### 7.4 Token en localStorage vs sessionStorage

| Situación | Comportamiento |
|---|---|
| F5 / recargar página | ✅ Funciona (localStorage) |
| Nueva pestaña | ✅ Funciona (localStorage) |
| Cerrar y abrir navegador | ✅ Funciona (localStorage) |
| Token expirado (>1h) | ✅ Refresh automático |
| Refresh expirado (>30d) | ❌ Login requerido |

### 7.5 Modo desarrollo (sin OIDC)

En desarrollo, el middleware usa `JwtValidator::dev()` que acepta cualquier
token. El endpoint `/auth/refresh` debe devolver el mismo token en modo dev:

```rust
if state.jwt_validator.is_dev() {
    return Ok(Json(RefreshResponse {
        access_token: "dev-token".to_string(),
        expires_in: 3600,
    }));
}
```

---

## 8. Checklist de implementación

### Backend

- [ ] Añadir campo `refresh_token: Option<String>` a `TokenResponse`
- [ ] Añadir campo `expires_in: u64` (quitar `#[allow(dead_code)]`)
- [ ] Crear tabla `refresh_tokens` en SQLite
- [ ] Implementar `save_refresh_token()`, `get_refresh_token()`, `delete_refresh_token()`
- [ ] Guardar refresh_token en el callback OIDC (`auth_routes.rs`)
- [ ] Implementar endpoint `POST /auth/refresh`
- [ ] Añadir `/auth/refresh` a `is_public_path()` en `main.rs`
- [ ] Manejar modo dev en `/auth/refresh`
- [ ] Tests: refresh exitoso, refresh expirado, refresh concurrente

### Frontend

- [ ] Guardar token en `localStorage` además de `sessionStorage`
- [ ] Implementar interceptor 401 con refresh en `http.ts`
- [ ] Variable `refreshPromise` para evitar refrescos concurrentes
- [ ] Reintentar petición original tras refresh exitoso
- [ ] Redirigir a login solo si refresh falla
- [ ] Tests: refresh exitoso, refresh fallido, concurrencia

---

## Apéndice A: Referencia de PocketID

| Recurso | URL |
|---|---|
| Repositorio | https://github.com/pocket-id/pocket-id |
| Documentación | https://pocket-id.org/docs |
| Token handler | `backend/internal/oidc/token_handler.go` |
| Config de tokens | `backend/internal/oidc/provider.go` (línea `RefreshTokenLifespan`) |
| Store (sesiones) | `backend/internal/oidc/store.go` |

## Apéndice B: Ejemplo de respuesta del token endpoint

```json
// POST /api/oidc/token (authorization_code)
{
  "access_token": "eyJhbGciOiJSUzI1NiIsImtpZCI6IjEifQ...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "def50200b5f8a4b7...",
  "id_token": "eyJhbGciOiJSUzI1NiIsImtpZCI6IjEifQ..."
}

// POST /api/oidc/token (refresh_token)
{
  "access_token": "eyJhbGciOiJSUzI1NiIsImtpZCI6IjEifQ...nuevo",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "def50200...nuevo"
}
```

## Apéndice C: Diagrama de secuencia

```
Frontend                    Backend                    PocketID
   │                          │                          │
   │── GET /api/feeds ────────│                          │
   │   Authorization: Bearer  │                          │
   │   (token expirado)       │                          │
   │                          │── valida JWT ────────────│
   │                          │   ← 401 (expirado)       │
   │← 401 Unauthorized ───────│                          │
   │                          │                          │
   │── POST /auth/refresh ────│                          │
   │                          │── POST /api/oidc/token ──│
   │                          │   grant_type=refresh     │
   │                          │   ← nuevos tokens ───────│
   │                          │                          │
   │                          │── guarda nuevo refresh ──│
   │                          │   (rotación)             │
   │← { access_token: "..." } │                          │
   │                          │                          │
   │── GET /api/feeds ────────│                          │
   │   Authorization: Bearer  │                          │
   │   (nuevo token)          │                          │
   │                          │── valida JWT ────────────│
   │                          │   ← OK                   │
   │← 200 { feeds: [...] } ───│                          │
```

---

> **Documento mantenido por**: El equipo de Populatrs
> **Próxima revisión**: Cuando se implemente el refresh token en Populatrs