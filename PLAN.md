# OIDC Refresh Token Support — Implementation Plan

## Objetivo

Implementar refresh token OIDC para que Populatrs no redirija al login al abrir una nueva pestaña, al expirar el JWT (~1h), al cerrar/reabrir el navegador, o tras inactividad de hasta 30 días.

## Arquitectura

El backend captura el `refresh_token` de PocketID durante el callback OIDC y lo almacena en una nueva tabla SQLite `refresh_tokens`. Un nuevo endpoint `POST /auth/refresh` canjea el refresh_token almacenado por un nuevo access_token (con rotación). El frontend guarda el access_token en `localStorage` (además de `sessionStorage`) e intercepta respuestas 401 para refrescar automáticamente antes de redirigir al login. Un `refreshPromise` compartido evita refrescos concurrentes.

## Tareas

### Tarea 1: Backend — TokenResponse + callback capture

**Archivos:**
- Modificar: `backend/src/routes/auth_routes.rs`

- [ ] **Paso 1: Añadir `refresh_token` a `TokenResponse`**
      El struct `TokenResponse` actual ignora `refresh_token`. Añadirlo como `Option<String>` para que serde lo deserialice cuando PocketID lo incluya.

      ```rust
      // Línea 36-43 — reemplazar struct TokenResponse
      #[derive(Debug, Deserialize)]
      struct TokenResponse {
          access_token: String,
          #[allow(dead_code)]
          token_type: String,
          expires_in: u64,
          id_token: Option<String>,
          refresh_token: Option<String>,  // ← NUEVO
      }
      ```

- [ ] **Paso 2: Guardar refresh_token en SQLite tras callback exitoso**
      Después de obtener `token_data` del token endpoint (línea 209), antes de generar el HTML de redirección, guardar el refresh_token en la base de datos. Extraer el `sub` del userinfo o del id_token para usarlo como `user_id`.

      ```rust
      // Después de la línea 217 (let access_token = token_data.access_token.clone();)
      // Guardar refresh_token si está presente
      if let Some(ref refresh_token) = token_data.refresh_token {
          let user_id = user_info
              .as_ref()
              .map(|u| u.sub.clone())
              .unwrap_or_else(|| {
                  // Fallback: extraer sub del id_token
                  token_data.id_token.as_ref().cloned().unwrap_or_default()
              });
          if !user_id.is_empty() {
              state
                  .db
                  .save_refresh_token(&user_id, refresh_token, token_data.expires_in)
                  .await
                  .ok();
          }
      }
      ```

- [ ] **Paso 3: Verificar compilación**
      ```bash
      cd backend && cargo check 2>&1 | head -30
      ```

---

### Tarea 2: Backend — SQLite refresh_tokens table + CRUD

**Archivos:**
- Modificar: `backend/src/db.rs`

- [ ] **Paso 1: Añadir migración para tabla `refresh_tokens`**
      En `run_migrations()` (línea 45), añadir la creación de la tabla dentro del bloque `execute_batch`:

      ```rust
      // Dentro del execute_batch en run_migrations(), después de la tabla settings (línea 112)
      CREATE TABLE IF NOT EXISTS refresh_tokens (
          user_id TEXT PRIMARY KEY,
          refresh_token TEXT NOT NULL,
          expires_at TEXT NOT NULL,
          created_at TEXT NOT NULL DEFAULT (datetime('now'))
      );
      ```

- [ ] **Paso 2: Implementar `save_refresh_token()`**
      Añadir después de la sección Settings (después de línea 939):

      ```rust
      // ───── Refresh Tokens ─────

      /// Save a refresh token for a user (upsert).
      pub async fn save_refresh_token(
          &self,
          user_id: &str,
          refresh_token: &str,
          expires_in: u64,
      ) -> Result<()> {
          let conn = self.conn.lock().await;
          let expires_at = (chrono::Utc::now() + chrono::Duration::seconds(expires_in as i64))
              .to_rfc3339();
          conn.execute(
              "INSERT INTO refresh_tokens (user_id, refresh_token, expires_at) \
               VALUES (?1, ?2, ?3) \
               ON CONFLICT(user_id) DO UPDATE SET \
               refresh_token = excluded.refresh_token, \
               expires_at = excluded.expires_at",
              params![user_id, refresh_token, expires_at],
          )
          .context("Failed to save refresh token")?;
          Ok(())
      }
      ```

- [ ] **Paso 3: Implementar `get_refresh_token()`**

      ```rust
      /// Get the stored refresh token for a user.
      pub async fn get_refresh_token(&self, user_id: &str) -> Result<Option<String>> {
          let conn = self.conn.lock().await;
          let token = conn
              .query_row(
                  "SELECT refresh_token FROM refresh_tokens WHERE user_id = ?1",
                  params![user_id],
                  |row| row.get::<_, String>(0),
              )
              .optional()
              .context("Failed to query refresh token")?;
          Ok(token)
      }
      ```

- [ ] **Paso 4: Implementar `delete_refresh_token()`**

      ```rust
      /// Delete a stored refresh token for a user.
      pub async fn delete_refresh_token(&self, user_id: &str) -> Result<()> {
          let conn = self.conn.lock().await;
          conn.execute(
              "DELETE FROM refresh_tokens WHERE user_id = ?1",
              params![user_id],
          )
          .context("Failed to delete refresh token")?;
          Ok(())
      }
      ```

- [ ] **Paso 5: Verificar compilación y tests**
      ```bash
      cd backend && cargo check 2>&1 | head -30
      cd backend && cargo test 2>&1 | tail -20
      ```

---

### Tarea 3: Backend — POST /auth/refresh endpoint

**Archivos:**
- Modificar: `backend/src/routes/auth_routes.rs`
- Modificar: `backend/src/routes/mod.rs`
- Modificar: `backend/src/main.rs`

- [ ] **Paso 1: Añadir struct `RefreshResponse` y función `refresh_token` en `auth_routes.rs`**
      Añadir después de la función `me()` (después de línea 301):

      ```rust
      #[derive(Debug, Serialize)]
      pub struct RefreshResponse {
          pub access_token: String,
          pub expires_in: u64,
      }

      /// Refresca el access_token usando el refresh_token almacenado en SQLite.
      ///
      /// 1. Extrae el user_id del token JWT actual (aunque esté expirado, el middleware
      ///    público lo pasa sin validar — el endpoint es público pero requiere token)
      /// 2. Lee el refresh_token de SQLite
      /// 3. Lo canjea en PocketID por un nuevo access_token + refresh_token (rotación)
      /// 4. Guarda el nuevo refresh_token
      /// 5. Devuelve el nuevo access_token
      ///
      /// En modo dev (sin OIDC), devuelve el mismo token.
      #[instrument(skip(state))]
      pub async fn refresh_token(
          State(state): State<Arc<AppState>>,
          Extension(user): Extension<AuthUser>,
      ) -> Result<Json<RefreshResponse>, StatusCode> {
          // Modo dev: devolver token de desarrollo
          if state.jwt_validator.is_dev() {
              return Ok(Json(RefreshResponse {
                  access_token: "dev-token".to_string(),
                  expires_in: 3600,
              }));
          }

          // 1. Obtener refresh_token de SQLite
          let refresh_token = state
              .db
              .get_refresh_token(&user.user_id)
              .await
              .map_err(|_| StatusCode::UNAUTHORIZED)?
              .ok_or(StatusCode::UNAUTHORIZED)?;

          // 2. Canjear en PocketID
          let issuer = state
              .config
              .oidc_issuer_url
              .as_deref()
              .ok_or(StatusCode::BAD_GATEWAY)?;
          let token_url = format!("{}/api/oidc/token", issuer.trim_end_matches('/'));
          let client_id = state.config.oidc_client_id.as_deref().unwrap_or("");
          let client_secret = state.config.oidc_client_secret.as_deref().unwrap_or("");

          let params = [
              ("grant_type", "refresh_token"),
              ("refresh_token", &refresh_token),
              ("client_id", client_id),
              ("client_secret", client_secret),
          ];

          let client = reqwest::Client::new();
          let resp = client
              .post(&token_url)
              .form(&params)
              .send()
              .await
              .map_err(|_| StatusCode::BAD_GATEWAY)?;

          if !resp.status().is_success() {
              // Refresh token expirado o inválido → limpiar y forzar login
              state.db.delete_refresh_token(&user.user_id).await.ok();
              return Err(StatusCode::UNAUTHORIZED);
          }

          let token_data: TokenResponse = resp
              .json()
              .await
              .map_err(|_| StatusCode::BAD_GATEWAY)?;

          // 3. Guardar nuevo refresh_token (rotación)
          if let Some(new_refresh) = &token_data.refresh_token {
              state
                  .db
                  .save_refresh_token(&user.user_id, new_refresh, token_data.expires_in)
                  .await
                  .ok();
          }

          // 4. Devolver nuevo access_token
          Ok(Json(RefreshResponse {
              access_token: token_data.access_token,
              expires_in: token_data.expires_in,
          }))
      }
      ```

- [ ] **Paso 2: Registrar ruta `/auth/refresh` en `routes/mod.rs`**
      Añadir en el router público (línea 34-39):

      ```rust
      // Línea 39 — después de .route("/auth/dev-login", ...)
      .route("/auth/refresh", routing::post(auth_routes::refresh_token))
      ```

- [ ] **Paso 3: Añadir `/auth/refresh` a `is_public_path()` en `main.rs`**
      La ruta ya está cubierta por `path.starts_with("/auth/")` (línea 362), pero añadir una entrada explícita para claridad y tests:

      ```rust
      // Línea 362 — añadir después de path.starts_with("/auth/")
      // Nota: /auth/refresh ya está cubierto por starts_with("/auth/"),
      // pero se añade explícitamente para claridad en tests
      ```

      No se necesita cambio — `path.starts_with("/auth/")` ya cubre `/auth/refresh`. Pero añadir un test:

      ```rust
      // En el bloque de tests (después de línea 460), añadir:
      #[test]
      fn test_is_public_path_auth_refresh() {
          assert!(is_public_path("/auth/refresh"));
      }
      ```

- [ ] **Paso 4: Verificar compilación**
      ```bash
      cd backend && cargo check 2>&1 | head -30
      cd backend && cargo test 2>&1 | tail -20
      ```

---

### Tarea 4: Frontend — localStorage + 401 interceptor

**Archivos:**
- Modificar: `frontend/src/store/auth.ts`
- Modificar: `frontend/src/api/http.ts`
- Modificar: `frontend/src/hooks/useAuth.ts`

- [ ] **Paso 1: Verificar que `setToken` ya guarda en `localStorage`**
      El archivo `frontend/src/store/auth.ts` ya guarda en ambos almacenes (líneas 9-14). No requiere cambios.

      ```typescript
      // ✅ Ya implementado — setToken guarda en sessionStorage y localStorage
      export function setToken(token: string): void {
        try {
          sessionStorage.setItem("populatrs_token", token);
          localStorage.setItem("populatrs_token", token);
        } catch { /* noop */ }
      }
      ```

- [ ] **Paso 2: Añadir `refreshAccessToken` y 401 interceptor en `http.ts`**
      En `frontend/src/api/http.ts`, añadir antes de la función `fetcher` (antes de línea 51):

      ```typescript
      import { getToken, setToken, clearToken } from "../store/auth";

      // Estado global para evitar múltiples refrescos concurrentes
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
      ```

      Luego modificar la función `fetcher` (líneas 51-69) para añadir el interceptor 401:

      ```typescript
      async function fetcher<T>(path: string, opts?: { method?: string; body?: unknown }): Promise<T> {
        const token = getToken();
        const headers: Record<string, string> = { "Content-Type": "application/json" };
        if (token) headers["Authorization"] = `Bearer ${token}`;

        const res = await fetch(path, {
          method: opts?.method ?? (opts?.body ? "POST" : "GET"),
          headers,
          body: opts?.body ? JSON.stringify(opts.body) : undefined,
        });

        // 🔄 Interceptor 401: intentar refresh antes de fallar
        if (res.status === 401 && token) {
          // Evitar múltiples refrescos concurrentes
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

- [ ] **Paso 3: Actualizar `useAuth` para no limpiar token en `fetchMe` failure**
      En `frontend/src/hooks/useAuth.ts`, modificar el `.catch()` (líneas 26-29) para no llamar a `clearToken()`:

      ```typescript
      // Líneas 24-30 — reemplazar el bloque .catch()
      fetchMe()
        .then(setUser)
        .catch(() => {
          // NO limpiar token aquí — el interceptor de 401 ya lo hizo
          // si el refresh falló. Si el error es de red, el token sigue
          // siendo válido y no debemos borrarlo.
          setUser(null);
        })
        .finally(() => setLoading(false));
      ```

      También eliminar el import de `clearToken` si ya no se usa:

      ```typescript
      // Línea 3 — eliminar clearToken del import
      import { fetchMe, type User } from "../api/http";
      // (getToken se mantiene porque se usa localmente)
      ```

- [ ] **Paso 4: Verificar build frontend**
      ```bash
      cd frontend && pnpm build 2>&1 | tail -20
      ```

---

### Tests de verificación

- [ ] **Backend: compilar y pasar tests existentes**
      ```bash
      cd backend && cargo test 2>&1 | tail -30
      ```

- [ ] **Frontend: compilar y pasar tests existentes**
      ```bash
      cd frontend && pnpm test 2>&1 | tail -30
      ```

- [ ] **Integración: flujo completo**
      1. Iniciar backend en modo dev: `cd backend && cargo run`
      2. Verificar que `POST /auth/refresh` devuelve `{"access_token":"dev-token","expires_in":3600}`
      3. Iniciar frontend: `cd frontend && pnpm dev`
      4. Hacer login, esperar a que el token expire (o simularlo borrando el token de sessionStorage)
      5. Verificar que la app refresca automáticamente sin redirigir al login