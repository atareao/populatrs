# PKCE (S256) en flujo OIDC — Plan de Implementación

## Objetivo

Añadir PKCE (Proof Key for Code Exchange) con S256 al flujo OIDC de login para eliminar la dependencia del `client_secret` en el exchange del authorization code, siguiendo el estándar RFC 7636.

## Arquitectura

El flujo OIDC actual usa `client_secret` en el token exchange. Con PKCE, el login handler genera un `code_verifier` aleatorio, calcula su `code_challenge` (SHA256 → base64url), y envía el challenge en la URL de authorize. El callback recupera el `code_verifier` almacenado y lo envía junto con `client_secret` en el token exchange. El tipo `OidcStates` se extiende para almacenar también el `code_verifier`.

## Archivos a modificar

| Archivo | Cambio |
|---|---|
| `backend/Cargo.toml` | Añadir `getrandom = "0.3"` |
| `backend/src/auth.rs` | Cambiar `OidcStates` a tupla de 3 campos |
| `backend/src/routes/auth_routes.rs` | Añadir helpers PKCE, modificar login/callback, actualizar tests |
| `backend/src/routes/oauth.rs` | Actualizar inserciones/destructuring al nuevo tipo |

## Tareas

### Tarea 1: Añadir dependencia getrandom

**Archivos:** `backend/Cargo.toml`

- [ ] **Paso 1.1:** Añadir `getrandom = "0.3"` en la sección `[dependencies]` de Cargo.toml.

### Tarea 2: Cambiar tipo OidcStates en auth.rs

**Archivos:** `backend/src/auth.rs:53-54`

- [ ] **Paso 2.1:** Cambiar la definición de `OidcStates` de:
  ```rust
  pub type OidcStates =
      Arc<tokio::sync::Mutex<std::collections::HashMap<String, (String, std::time::Instant)>>>;
  ```
  a:
  ```rust
  pub type OidcStates =
      Arc<tokio::sync::Mutex<std::collections::HashMap<String, (String, String, std::time::Instant)>>>;
  ```
  El tuple pasa a ser `(state, code_verifier, timestamp)`.

### Tarea 3: Actualizar oauth.rs para el nuevo tipo

**Archivos:** `backend/src/routes/oauth.rs`

- [ ] **Paso 3.1:** En todas las inserciones a `oauth_states`, cambiar de `(oauth_state, Instant::now())` a `(oauth_state, String::new(), Instant::now())`.

- [ ] **Paso 3.2:** En todas las comparaciones de estado, cambiar el patrón de destructuring de `(ref stored_state, _)` a `(ref stored_state, _, _)`.

- [ ] **Paso 3.3:** En `resolve_publisher_id`, cambiar la firma y destructuring para la tupla de 3 campos.

- [ ] **Paso 3.4:** En los tests de oauth.rs, actualizar las tuplas de 2 campos a 3 campos.

### Tarea 4: Añadir helpers PKCE en auth_routes.rs

**Archivos:** `backend/src/routes/auth_routes.rs`

- [ ] **Paso 4.1:** Añadir imports necesarios (`sha2`, `base64::Engine`).

- [ ] **Paso 4.2:** Añadir `generate_code_verifier()` — 48 bytes aleatorios → base64url → 64 chars.

- [ ] **Paso 4.3:** Añadir `compute_code_challenge(verifier)` — SHA256(verifier) → base64url.

### Tarea 5: Modificar login() para PKCE

**Archivos:** `backend/src/routes/auth_routes.rs:52-88`

- [ ] **Paso 5.1:** Generar `code_verifier` y `code_challenge` después de `oauth_state`.

- [ ] **Paso 5.2:** Guardar `(oauth_state, code_verifier, Instant::now())` en `oidc_states`.

- [ ] **Paso 5.3:** Añadir `code_challenge_method=S256` y `code_challenge` a la URL de authorize.

### Tarea 6: Modificar callback() para usar code_verifier

**Archivos:** `backend/src/routes/auth_routes.rs:90-286`

- [ ] **Paso 6.1:** Cambiar destructuring de stored state a 3 campos.

- [ ] **Paso 6.2:** Extraer `code_verifier` del stored state.

- [ ] **Paso 6.3:** Añadir `code_verifier` al form de token exchange.

### Tarea 7: Actualizar tests existentes

**Archivos:** `backend/src/routes/auth_routes.rs` (sección `#[cfg(test)]`)

- [ ] **Paso 7.1:** Actualizar `test_state_equality` a tupla de 3 campos.

- [ ] **Paso 7.2:** Actualizar `test_state_mismatch` a tupla de 3 campos.

### Tarea 8: Añadir tests para PKCE

**Archivos:** `backend/src/routes/auth_routes.rs` (sección `#[cfg(test)]`)

- [ ] **Paso 8.1:** `test_generate_code_verifier_length` — verifica 64 chars, solo base64url.

- [ ] **Paso 8.2:** `test_compute_code_challenge` — verifica determinismo y formato.

- [ ] **Paso 8.3:** `test_login_url_includes_pkce_params` — verifica code_challenge en URL.

### Tarea 9: Verificar compilación y tests

- [ ] **Paso 9.1:** Ejecutar `cargo check`.
- [ ] **Paso 9.2:** Ejecutar `cargo test`.
- [ ] **Paso 9.3:** Ejecutar `cargo clippy -- -D warnings`.

## Criterios de verificación

1. **Compilación**: `cargo check` pasa sin errores ni warnings.
2. **Tests**: `cargo test` pasa — todos los tests existentes más los 3 nuevos.
3. **Clippy**: `cargo clippy -- -D warnings` pasa sin infracciones.
4. **Funcional**: Login OIDC genera URL con `code_challenge_method=S256`; callback envía `code_verifier` en el exchange.
5. **Regresión**: El flujo OAuth de publishers (oauth.rs) sigue funcionando con `String::new()` como code_verifier.