# Reconnect OAuth Publishers — Implementation Plan

## Objetivo

Añadir un botón "Reconnect" en los publishers OAuth (X, LinkedIn, Threads, Mastodon) que ya están conectados, permitiendo re-ejecutar el flujo OAuth para renovar tokens, junto con información visual sobre el estado de expiración.

## Arquitectura

- **Backend**: Nuevo endpoint `GET /api/publishers/{id}/oauth/status` que inspecciona la configuración del publisher desde SQLite y devuelve `connected`, `token_expires_at`, `has_refresh_token` y `publisher_type`.
- **Frontend**: Nueva función `fetchOAuthStatus` en la capa API. El componente `PublisherList` muestra un botón "Reconnect" (que reusa el popup OAuth existente) e información de expiración para publishers conectados.
- No se modifican los flujos OAuth existentes (authorize, callback, popup postMessage) — solo se añade un nuevo endpoint de consulta y UI.

## Tareas

### Tarea 1: Backend — Añadir endpoint `GET /api/publishers/{id}/oauth/status`

**Archivos:**
- Modificar: `backend/src/routes/oauth.rs` (añadir función `status`)

- [ ] **Paso 1:** Añadir la función `pub async fn status()` en `backend/src/routes/oauth.rs`
  - Extraer `State(state)` y `Path(id)` como los demás handlers
  - Cargar el publisher con `state.db.get_publisher(&id).await`
  - Si no existe, devolver `404` con `{ "ok": false, "error": "Publisher not found" }`
  - Hacer `match` sobre el `PublisherConfig` para determinar:
    - `connected`: `access_token.is_some()` (para X, LinkedIn, Threads, Mastodon)
    - `token_expires_at`: el campo `token_expires_at` (solo Threads lo tiene)
    - `has_refresh_token`: `refresh_token.is_some()` (para X, LinkedIn)
    - `publisher_type`: `config.type_name()`
  - Para tipos no-OAuth, devolver `connected: false`
  - Devolver `Json(json!({ "connected": bool, "token_expires_at": Option<i64>, "has_refresh_token": bool, "publisher_type": String }))`

  ```rust
  pub async fn status(
      State(state): State<Arc<AppState>>,
      Path(id): Path<String>,
  ) -> impl IntoResponse {
      let config = match state.db.get_publisher(&id).await {
          Ok(Some(c)) => c,
          Ok(None) => {
              return (StatusCode::NOT_FOUND, Json(json!({
                  "ok": false, "error": "Publisher not found"
              }))).into_response();
          }
          Err(e) => {
              return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                  "ok": false, "error": format!("Database error: {e}")
              }))).into_response();
          }
      };

      let (connected, token_expires_at, has_refresh_token) = match &config {
          PublisherConfig::X { access_token, refresh_token, .. } => (
              access_token.is_some(),
              None,
              refresh_token.is_some(),
          ),
          PublisherConfig::LinkedIn { access_token, refresh_token, .. } => (
              access_token.is_some(),
              None,
              refresh_token.is_some(),
          ),
          PublisherConfig::Threads { access_token, token_expires_at, .. } => (
              access_token.is_some(),
              *token_expires_at,
              false,
          ),
          PublisherConfig::Mastodon { access_token, .. } => (
              access_token.is_some(),
              None,
              false,
          ),
          _ => (false, None, false),
      };

      Json(json!({
          "connected": connected,
          "token_expires_at": token_expires_at,
          "has_refresh_token": has_refresh_token,
          "publisher_type": config.type_name(),
      })).into_response()
  }
  ```

### Tarea 2: Backend — Registrar la nueva ruta

**Archivos:**
- Modificar: `backend/src/routes/mod.rs`

- [ ] **Paso 1:** Añadir la ruta en el bloque `protected` de `api_routes()`, junto a las rutas OAuth existentes:

  ```rust
  .route(
      "/api/publishers/{id}/oauth/status",
      routing::get(oauth::status),
  )
  ```

  Colocarla justo después de la ruta `oauth::callback` (línea 77).

### Tarea 3: Frontend — Añadir `fetchOAuthStatus` a la capa API

**Archivos:**
- Modificar: `frontend/src/api/http.ts`

- [ ] **Paso 1:** Añadir el tipo de retorno y la función después de `oauthCallback` (tras línea 161):

  ```typescript
  export interface OAuthStatus {
    connected: boolean;
    token_expires_at: number | null;
    has_refresh_token: boolean;
    publisher_type: string;
  }

  export async function fetchOAuthStatus(id: string): Promise<OAuthStatus> {
    return fetcher(`/api/publishers/${id}/oauth/status`);
  }
  ```

### Tarea 4: Frontend — Actualizar `PublisherList.tsx` con Reconnect y token status

**Archivos:**
- Modificar: `frontend/src/pages/Publishers/PublisherList.tsx`

- [ ] **Paso 1:** Añadir `fetchOAuthStatus` al import de `../../api/http` (línea 7):

  ```typescript
  import {
    fetchPublishers, createPublisher, updatePublisher, testPublisher, deletePublisher, getOAuthUrl,
    fetchOAuthStatus,
    type PublisherConfigEntry,
  } from "../../api/http";
  ```

- [ ] **Paso 2:** Añadir estado para almacenar los OAuth status por publisher ID. Añadir tras la línea 84 (`const [form] = Form.useForm();`):

  ```typescript
  const [oauthStatuses, setOauthStatuses] = useState<Record<string, { connected: boolean; token_expires_at: number | null; has_refresh_token: boolean }>>({});
  ```

- [ ] **Paso 3:** En `loadData()` (línea 86), después de cargar publishers, disparar `fetchOAuthStatus` para cada publisher OAuth conectado. Añadir tras `setPublishers(data.publishers)` (línea 89):

  ```typescript
  // Fetch OAuth status for connected OAuth publishers
  const statusPromises = Object.entries(data.publishers)
    .filter(([_, cfg]) => ["X", "LinkedIn", "Threads", "Mastodon"].includes(cfg.type) && cfg.config.access_token)
    .map(async ([id]) => {
      try {
        const status = await fetchOAuthStatus(id);
        return [id, status] as const;
      } catch { return null; }
    });
  const results = await Promise.all(statusPromises);
  const statusMap: Record<string, typeof oauthStatuses[string]> = {};
  for (const r of results) {
    if (r) statusMap[r[0]] = r[1];
  }
  setOauthStatuses(statusMap);
  ```

- [ ] **Paso 4:** En la columna "Actions", modificar el bloque OAuth (líneas 260-268) para mostrar Reconnect + token info cuando ya está conectado:

  ```typescript
  {isOAuthType && (
    connected
      ? (
        <Space size="small">
          <Tag color="green">Connected</Tag>
          {oauthStatuses[record.id]?.token_expires_at && (
            <Tag color="orange">
              Expires {Math.round((oauthStatuses[record.id].token_expires_at! - Date.now() / 1000) / 86400)} days
            </Tag>
          )}
          <Button size="small" onClick={() => handleOAuth(record.id, type)}>
            Reconnect
          </Button>
        </Space>
      )
      : (
        <Button size="small" type="primary" onClick={() => handleOAuth(record.id, type)}>
          Connect
        </Button>
      )
  )}
  ```

  El botón "Reconnect" reusa la misma función `handleOAuth` que abre el popup OAuth. El flujo postMessage existente recargará la lista automáticamente al completarse.

## Verification

1. **Backend test**: Arrancar el servidor en modo desarrollo y llamar a:
   ```bash
   curl -s http://localhost:3044/api/publishers/{id}/oauth/status \
     -H "Authorization: Bearer dev-token"
   ```
   Verificar que devuelve `connected`, `token_expires_at`, `has_refresh_token`, `publisher_type`.

2. **Frontend test**: Abrir la página de Publishers. Para un publisher X/LinkedIn/Threads/Mastodon conectado:
   - Ver el tag "Connected" + botón "Reconnect"
   - Para Threads, ver el tag "Expires N days"
   - Hacer clic en "Reconnect" → debe abrir el popup OAuth
   - Completar el flujo OAuth → el popup se cierra y la lista se recarga

3. **Regresión**: Verificar que publishers no-OAuth (Telegram, Discord, etc.) no muestran cambios en su UI.