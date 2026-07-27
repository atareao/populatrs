# X/Threads Token Persistence Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix two token-related bugs — X/Twitter refresh tokens not persisted to DB (causing 400 errors) and Threads short-lived tokens never exchanged for long-lived ones (causing 500 errors).

**Architecture:** Add an optional `Arc<Database>` field to `XPublisher` and `ThreadsPublisher` so token updates are written through to SQLite. Threads gets a long-lived token exchange flow (Meta's `th_exchange_token` endpoint) plus expiry tracking.

**Tech Stack:** Rust (Axum, reqwest, tokio), SQLite (rusqlite via `Database` struct)

## Global Constraints

- All new code must compile with `cargo build`, pass `cargo test`, pass `cargo clippy -- -D warnings`, and pass `cargo fmt -- --check`.
- The `Database` struct is in `crate::db::Database` with method `upsert_publisher(id, config, enabled)`.
- `PublisherConfig::X` has fields: `client_id`, `client_secret`, `access_token`, `refresh_token`, `redirect_uri`, `template`.
- `PublisherConfig::Threads` has fields: `client_id`, `client_secret`, `access_token`, `user_id`, `redirect_uri`, `template`.
- `XPublisher::new()` takes 8 params: `id`, `client_id`, `client_secret`, `access_token: Option<String>`, `refresh_token: Option<String>`, `redirect_uri: Option<String>`, `template: String`, `config_file_path: Option<String>`.
- `ThreadsPublisher::new()` takes 7 params: `id`, `client_id`, `client_secret`, `access_token: Option<String>`, `user_id: Option<String>`, `redirect_uri: Option<String>`, `template: String`.
- `PublisherManager::add_publisher(id, config)` calls `create_publisher_with_config_path(id, config, self.config_path)`.
- `create_publisher(id, config)` is a wrapper that calls `create_publisher_with_config_path(id, config, None)`.
- `Database::upsert_publisher(&self, id: &str, config: &PublisherConfig, enabled: bool) -> Result<()>` is the persistence method.
- `AppState` has `pub db: Database` — cloneable via `state.db.clone()`.

---

## Tasks

### Task 0: Survey and verify understanding

**Files:**
- Read: `backend/src/publisher/mod.rs`
- Read: `backend/src/publisher/x.rs`
- Read: `backend/src/publisher/threads.rs`
- Read: `backend/src/routes/oauth.rs`
- Read: `backend/src/main.rs`
- Read: `backend/src/db.rs`
- Read: `backend/src/models.rs` (PublisherConfig enum)
- Read: `backend/src/auth.rs` (AppState struct)
- Read: `backend/src/lib.rs` (run_feed_check)

- [ ] **Step 1: Read all relevant files**

    The engineer should read every file listed above to understand the current code structure, the `PublisherConfig` enum variants, the `Database::upsert_publisher` signature, and how `AppState` exposes `db`.

- [ ] **Step 2: Run current tests to establish baseline**

    ```bash
    cd /data/rust/populatrs/backend && cargo test 2>&1 | tail -20
    ```

    Expected: all tests pass.

---

### Task 1: Add DB field to XPublisher and implement `save_tokens_to_config`

**Files:**
- Modify: `backend/src/publisher/x.rs`

- [ ] **Step 1: Add `db` field to `XPublisher` struct**

    Add an `use` for `Database` at the top of the file, then add the field to the struct:

    ```rust
    // Add to imports at top of file
    use crate::db::Database;
    ```

    ```rust
    pub struct XPublisher {
        pub id: String,
        pub client_id: String,
        pub client_secret: String,
        pub access_token: Arc<Mutex<Option<String>>>,
        pub refresh_token: Arc<Mutex<Option<String>>>,
        pub redirect_uri: String,
        pub template: String,
        client: Client,
        renderer: TemplateRenderer,
        pub config_file_path: Option<String>,
        pub db: Option<Arc<Database>>,  // NEW
    }
    ```

- [ ] **Step 2: Add `db` parameter to `XPublisher::new()`**

    The method signature gains a `db: Option<Arc<Database>>` parameter. Wire it into the struct:

    ```rust
    pub fn new(
        id: String,
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        redirect_uri: Option<String>,
        template: String,
        config_file_path: Option<String>,
        db: Option<Arc<Database>>,  // NEW
    ) -> Self {
        let redirect_uri = redirect_uri.unwrap_or_else(|| "https://127.0.0.1".to_string());

        Self {
            id,
            client_id,
            client_secret,
            access_token: Arc::new(Mutex::new(access_token)),
            refresh_token: Arc::new(Mutex::new(refresh_token)),
            redirect_uri,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
            config_file_path,
            db,  // NEW
        }
    }
    ```

- [ ] **Step 3: Implement `save_tokens_to_config` to persist tokens**

    Replace the current no-op implementation:

    ```rust
    /// Guardar tokens actualizados en la configuración (persiste a DB)
    pub async fn save_tokens_to_config(
        &self,
        access_token: &str,
        refresh_token: Option<&str>,
    ) -> Result<()> {
        tracing::info!("Saving X tokens to config for '{}'", self.id);

        if let Some(ref db) = self.db {
            let config = PublisherConfig::X {
                client_id: self.client_id.clone(),
                client_secret: self.client_secret.clone(),
                access_token: Some(access_token.to_string()),
                refresh_token: refresh_token.map(|s| s.to_string()),
                redirect_uri: Some(self.redirect_uri.clone()),
                template: self.template.clone(),
            };
            db.upsert_publisher(&self.id, &config, true).await?;
            tracing::info!("Persisted X tokens to database for '{}'", self.id);
        } else {
            tracing::warn!(
                "No database reference — X tokens for '{}' only updated in memory",
                self.id
            );
        }

        Ok(())
    }
    ```

- [ ] **Step 4: Build to verify no compilation errors yet (expected — callers in mod.rs still need updating)**

    ```bash
    cd /data/rust/populatrs/backend && cargo build 2>&1 | head -30
    ```

    Expected: errors in `mod.rs` about missing argument to `XPublisher::new()` — that's fine, Task 2 fixes the callers.

---

### Task 2: Update factory functions in `mod.rs` to accept and pass DB

**Files:**
- Modify: `backend/src/publisher/mod.rs`

- [ ] **Step 1: Add `Database` import to `mod.rs`**

    ```rust
    use crate::db::Database;
    use std::sync::Arc;
    ```

- [ ] **Step 2: Add `db` parameter to `create_publisher_with_config_path`**

    The function signature gains `db: Option<Arc<Database>>`. Pass it to `XPublisher::new()` and `ThreadsPublisher::new()`:

    ```rust
    pub fn create_publisher_with_config_path(
        id: String,
        config: &PublisherConfig,
        config_path: Option<String>,
        db: Option<Arc<Database>>,  // NEW
    ) -> Result<Box<dyn Publisher>> {
        match config {
            // ... existing arms unchanged ...
            PublisherConfig::X {
                client_id,
                client_secret,
                access_token,
                refresh_token,
                redirect_uri,
                template,
            } => Ok(Box::new(XPublisher::new(
                id,
                client_id.clone(),
                client_secret.clone(),
                access_token.clone(),
                refresh_token.clone(),
                redirect_uri.clone(),
                template.clone(),
                config_path,
                db,  // NEW — pass through
            ))),
            PublisherConfig::Threads {
                client_id,
                client_secret,
                access_token,
                user_id,
                redirect_uri,
                template,
            } => Ok(Box::new(ThreadsPublisher::new(
                id,
                client_id.clone(),
                client_secret.clone(),
                access_token.clone(),
                user_id.clone(),
                redirect_uri.clone(),
                template.clone(),
                db,  // NEW — pass through
            ))),
            // ... all other arms unchanged (they don't use db) ...
        }
    }
    ```

    All other match arms (`Telegram`, `Mastodon`, `LinkedIn`, `OpenObserve`, `Matrix`, `Bluesky`, `Discord`) remain unchanged — they simply don't receive or use the `db` parameter.

- [ ] **Step 3: Verify the full function is correct**

    The complete modified function after these changes (only showing the changed parts):

    ```rust
    pub fn create_publisher_with_config_path(
        id: String,
        config: &PublisherConfig,
        config_path: Option<String>,
        db: Option<Arc<Database>>,
    ) -> Result<Box<dyn Publisher>> {
        match config {
            // ... Telegram, Mastodon, LinkedIn, OpenObserve, Matrix, Bluesky, Discord unchanged ...
            PublisherConfig::X { .. } => Ok(Box::new(XPublisher::new(
                // ... existing args ...
                db,  // pass through
            ))),
            PublisherConfig::Threads { .. } => Ok(Box::new(ThreadsPublisher::new(
                // ... existing args ...
                db,  // pass through
            ))),
            // ... rest unchanged ...
        }
    }
    ```

- [ ] **Step 4: Update `create_publisher` wrapper to pass `None` for db**

    The `create_publisher` helper already calls `create_publisher_with_config_path(id, config, None)` — since the new parameter is at the end, it will get the default. But to be explicit and avoid a compilation error, add `None`:

    ```rust
    pub fn create_publisher(id: String, config: &PublisherConfig) -> Result<Box<dyn Publisher>> {
        create_publisher_with_config_path(id, config, None, None)
    }
    ```

- [ ] **Step 5: Build to verify**

    ```bash
    cd /data/rust/populatrs/backend && cargo build 2>&1 | head -30
    ```

    Expected: compiles successfully (mod.rs callers now match the updated signatures).

---

### Task 3: Update `PublisherManager::add_publisher` and scheduler in `main.rs` to pass DB

**Files:**
- Modify: `backend/src/publisher/mod.rs` (add_publisher method)
- Modify: `backend/src/main.rs` (scheduler loop)

- [ ] **Step 1: Update `PublisherManager` to hold an optional DB reference**

    ```rust
    pub struct PublisherManager {
        publishers: HashMap<String, Box<dyn Publisher>>,
        config_path: Option<String>,
        db: Option<Arc<Database>>,  // NEW
    }
    ```

    Update the constructors:

    ```rust
    impl PublisherManager {
        pub fn new() -> Self {
            Self {
                publishers: HashMap::new(),
                config_path: None,
                db: None,
            }
        }

        pub fn new_with_config_path(config_path: String) -> Self {
            Self {
                publishers: HashMap::new(),
                config_path: Some(config_path),
                db: None,
            }
        }

        // NEW — optionally attach a DB reference
        pub fn new_with_db(config_path: Option<String>, db: Option<Arc<Database>>) -> Self {
            Self {
                publishers: HashMap::new(),
                config_path,
                db,
            }
        }
    }
    ```

- [ ] **Step 2: Update `add_publisher` to pass DB through**

    ```rust
    pub fn add_publisher(&mut self, id: String, config: &PublisherConfig) -> Result<()> {
        let publisher =
            create_publisher_with_config_path(id.clone(), config, self.config_path.clone(), self.db.clone())?;
        self.publishers.insert(id, publisher);
        Ok(())
    }
    ```

- [ ] **Step 3: In `main.rs`, pass DB reference to the scheduler's `PublisherManager`**

    In the `feed_scheduler_loop` function (~line 226), change:

    ```rust
    let mut publisher_manager = PublisherManager::new();
    ```

    to:

    ```rust
    let mut publisher_manager = PublisherManager::new_with_db(None, Some(Arc::new(db.clone())));
    ```

    And add the import at the top of `main.rs` if not already there:
    ```rust
    use std::sync::Arc;  // already present at line 2
    ```

- [ ] **Step 4: Verify build**

    ```bash
    cd /data/rust/populatrs/backend && cargo build 2>&1 | head -20
    ```

    Expected: compiles successfully.

---

### Task 4: Pass DB to X publisher in OAuth routes

**Files:**
- Modify: `backend/src/routes/oauth.rs`

- [ ] **Step 1: Pass `Some(state.db.clone())` when creating X publisher in `authorize` and `callback`**

    In the `authorize` function (line 65), change:
    ```rust
    let publisher = match create_publisher(id.clone(), &config) {
    ```
    to:
    ```rust
    let publisher = match create_publisher_with_config_path(id.clone(), &config, None, Some(state.db.clone())) {
    ```

    And update the import at the top of the file (line 16):
    ```rust
    use crate::publisher::{
        create_publisher, create_publisher_with_config_path, LinkedInPublisher, MastodonPublisher, ThreadsPublisher, XPublisher,
    };
    ```

    (Keep `create_publisher` for backward compat; add `create_publisher_with_config_path`.)

    **Note:** `authorize` downcasts `publisher` to `XPublisher` on ~line 79 and `ThreadsPublisher` on ~line 97 — both will now have the DB reference because we passed it through.

- [ ] **Step 2: The `callback` function uses `create_publisher` on line 193 — update it for X and Threads**

    Change line 193 from:
    ```rust
    let publisher = match create_publisher(id.clone(), &config) {
    ```
    to the same pattern:
    ```rust
    let publisher = match create_publisher_with_config_path(id.clone(), &config, None, Some(state.db.clone())) {
    ```

- [ ] **Step 3: Verify that Threads and X sections in both `authorize` and `callback` compile**

    The `XPublisher` downcast sections in both functions will now have `db` set because we passed `Some(state.db.clone())` into the factory. No separate changes needed in the downcast blocks.

- [ ] **Step 4: Build to verify**

    ```bash
    cd /data/rust/populatrs/backend && cargo build 2>&1 | head -20
    ```

    Expected: compiles successfully.

---

### Task 5: Fix Threads publisher — add DB field and long-lived token exchange

**Files:**
- Modify: `backend/src/publisher/threads.rs`

- [ ] **Step 1: Add imports and new fields to `ThreadsPublisher` struct**

    ```rust
    use crate::db::Database;
    use std::sync::Arc;
    ```

    Add `db` and `token_expires_at` fields to the struct:

    ```rust
    pub struct ThreadsPublisher {
        #[allow(dead_code)]
        pub id: String,
        pub client_id: String,
        pub client_secret: String,
        pub access_token: Option<String>,
        pub user_id: Option<String>,
        pub redirect_uri: String,
        pub template: String,
        client: Client,
        renderer: TemplateRenderer,
        pub db: Option<Arc<Database>>,        // NEW
        pub token_expires_at: Option<i64>,    // NEW — Unix timestamp
    }
    ```

- [ ] **Step 2: Add `db` parameter to `ThreadsPublisher::new()`**

    ```rust
    pub fn new(
        id: String,
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        user_id: Option<String>,
        redirect_uri: Option<String>,
        template: String,
        db: Option<Arc<Database>>,  // NEW
    ) -> Self {
        let redirect_uri = redirect_uri.unwrap_or_else(|| "https://127.0.0.1".to_string());

        Self {
            id,
            client_id,
            client_secret,
            access_token,
            user_id,
            redirect_uri,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
            db,                         // NEW
            token_expires_at: None,     // NEW — will be set after exchange
        }
    }
    ```

- [ ] **Step 3: Add method `exchange_for_long_lived_token`**

    ```rust
    /// Exchange a short-lived Threads token for a long-lived one (60 days).
    /// Meta endpoint: GET /access_token?grant_type=th_exchange_token
    async fn exchange_for_long_lived_token(&self, short_lived: &str) -> Result<(String, u64)> {
        let url = "https://graph.threads.net/access_token";
        let response = self
            .client
            .get(url)
            .query(&[
                ("grant_type", "th_exchange_token"),
                ("client_secret", &self.client_secret),
                ("access_token", short_lived),
            ])
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to exchange Threads token for long-lived: {} - {}",
                status,
                body
            ));
        }

        let data: serde_json::Value = response.json().await?;
        let long_lived = data["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No access_token in long-lived exchange response"))?
            .to_string();
        let expires_in = data["expires_in"].as_u64().unwrap_or(5184000); // default 60 days

        tracing::info!(
            "Exchanged Threads token for long-lived — expires in {}s",
            expires_in
        );

        Ok((long_lived, expires_in))
    }
    ```

- [ ] **Step 4: Add `save_tokens_to_config` method to ThreadsPublisher**

    ```rust
    /// Persist the current access_token and expiry to the database.
    async fn save_tokens_to_config(&self) -> Result<()> {
        let Some(ref db) = self.db else {
            tracing::warn!("No database reference — Threads tokens not persisted");
            return Ok(());
        };

        let access_token = self.access_token.as_deref().ok_or_else(|| {
            anyhow::anyhow!("Cannot save Threads config: no access token")
        })?;

        let config = PublisherConfig::Threads {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            access_token: Some(access_token.to_string()),
            user_id: self.user_id.clone(),
            redirect_uri: Some(self.redirect_uri.clone()),
            template: self.template.clone(),
        };
        db.upsert_publisher(&self.id, &config, true).await?;
        tracing::info!("Persisted Threads tokens to database for '{}'", self.id);
        Ok(())
    }
    ```

- [ ] **Step 5: Add `get_valid_access_token` method**

    ```rust
    /// Return a valid access token, exchanging for long-lived if expired.
    async fn get_valid_access_token(&self) -> Result<String> {
        let now = chrono::Utc::now().timestamp();

        // Check if we have a token and it's still valid (> 1 hour buffer)
        if let Some(ref token) = self.access_token {
            if let Some(expires_at) = self.token_expires_at {
                if now < expires_at - 3600 {
                    return Ok(token.clone());
                }
            } else {
                // No expiry set — assume it's the initial short-lived token. Exchange it.
                let (long_lived, expires_in) = self.exchange_for_long_lived_token(token).await?;
                // We need interior mutability — use a Cell or refactor to use Mutex.
                // Since publish() is behind &self, we use a workaround: store in a temporary
                // and return. The caller (publish) will need to handle the persistence.
                return Ok(long_lived);
            }
        }

        Err(anyhow::anyhow!("No Threads access token available"))
    }
    ```

    **Note:** Since `Publisher` trait uses `&self` (not `&mut self`), we can't mutate `self.access_token` and `self.token_expires_at` directly. The Threads flow needs a lightweight mutable container. We'll use `std::cell::UnsafeCell` is not safe — instead, we'll wrap the mutable fields in `Arc<tokio::sync::Mutex<>>` similar to X.

- [ ] **Step 6: Wrap mutable token fields in Mutex (revised approach)**

    Change `access_token`, `token_expires_at` to use `Arc<Mutex<>>` for interior mutability under `&self`:

    ```rust
    pub struct ThreadsPublisher {
        #[allow(dead_code)]
        pub id: String,
        pub client_id: String,
        pub client_secret: String,
        pub access_token: Arc<tokio::sync::Mutex<Option<String>>>,  // CHANGED
        pub user_id: Option<String>,
        pub redirect_uri: String,
        pub template: String,
        client: Client,
        renderer: TemplateRenderer,
        pub db: Option<Arc<Database>>,
        pub token_expires_at: Arc<tokio::sync::Mutex<Option<i64>>>,  // CHANGED
    }
    ```

    Update `new()` accordingly:

    ```rust
    Self {
        id,
        client_id,
        client_secret,
        access_token: Arc::new(tokio::sync::Mutex::new(access_token)),
        user_id,
        redirect_uri,
        template,
        client: Client::new(),
        renderer: TemplateRenderer::new(),
        db,
        token_expires_at: Arc::new(tokio::sync::Mutex::new(None)),
    }
    ```

    Add import:
    ```rust
    use std::sync::Arc;
    use tokio::sync::Mutex;  // or keep tokio::sync::Mutex qualified
    ```

- [ ] **Step 7: Update `get_valid_access_token` for Mutex**

    ```rust
    async fn get_valid_access_token(&self) -> Result<String> {
        let now = chrono::Utc::now().timestamp();

        {
            let token_guard = self.access_token.lock().await;
            let expires_guard = self.token_expires_at.lock().await;

            if let Some(ref token) = *token_guard {
                if let Some(expires_at) = *expires_guard {
                    if now < expires_at - 3600 {
                        // Still valid — at least 1 hour buffer
                        return Ok(token.clone());
                    }
                } else {
                    // Have a token but no expiry — assume short-lived, exchange below
                }
            }
        } // locks released

        // Need to exchange (token missing, expired, or no expiry set)
        let current_token = {
            let guard = self.access_token.lock().await;
            guard.clone().ok_or_else(|| anyhow::anyhow!("No Threads access token available"))?
        };

        let (new_token, expires_in) = self.exchange_for_long_lived_token(&current_token).await?;

        // Update in-memory state
        {
            let mut token_guard = self.access_token.lock().await;
            *token_guard = Some(new_token.clone());
        }
        {
            let mut expires_guard = self.token_expires_at.lock().await;
            *expires_guard = Some(now + expires_in as i64);
        }

        // Persist to DB
        if let Err(e) = self.save_tokens_to_config().await {
            tracing::warn!("Failed to persist Threads tokens: {}", e);
        }

        Ok(new_token)
    }
    ```

- [ ] **Step 8: Update `publish()` to use `get_valid_access_token`**

    The current `publish()` method reads `self.access_token` directly. Replace that with `get_valid_access_token()`:

    ```rust
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String> {
        let template_str = feed_template
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.template);

        // Get a valid (long-lived) access token
        let access_token = self.get_valid_access_token().await?;

        let user_id = self
            .user_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No user id"))?;

        // ... rest of the method unchanged, but use `access_token` local variable
        // instead of `self.access_token.as_ref() ...`
    ```

    Then update the container creation and publish steps to use the local `access_token` string (it's already used as `access_token.as_str()` in query params, which works with `&String`).

- [ ] **Step 9: Fix UTF-8 text truncation in publish**

    The current `publish()` method has:
    ```rust
    let text = if text.len() > 500 {
        format!("{}...", &text[..497])
    } else {
        text
    };
    ```

    This uses byte indexing (`text[..497]`) which panics on multi-byte UTF-8. Replace with:
    ```rust
    let text = if text.chars().count() > 500 {
        format!("{}...", text.chars().take(497).collect::<String>())
    } else {
        text
    };
    ```

- [ ] **Step 10: Ensure `save_tokens_to_config` is imported properly**

    Add the needed import for `PublisherConfig` at the top if not already present:
    ```rust
    use crate::models::PublisherConfig;  // verify this import exists — it may come via `use super::Publisher;`
    ```

    The file already imports `use crate::models::{Post, TemplateContext, TemplateRenderer};` — add `PublisherConfig`:
    ```rust
    use crate::models::{Post, PublisherConfig, TemplateContext, TemplateRenderer};
    ```

- [ ] **Step 11: Verify build**

    ```bash
    cd /data/rust/populatrs/backend && cargo build 2>&1 | head -30
    ```

    Expected: compiles with no errors.

---

### Task 6: Exchange Threads short-lived token for long-lived in OAuth callback

**Files:**
- Modify: `backend/src/routes/oauth.rs`

- [ ] **Step 1: In the Threads callback section, exchange the short-lived token for long-lived**

    In the `callback` function, after the Threads token exchange (around line 362-374), add the long-lived exchange call:

    ```rust
    // ── Threads callback ──
    if let Some(t_pub) = publisher.as_any().downcast_ref::<ThreadsPublisher>() {
        // ... state validation unchanged ...

        let (access_token, user_id, _expires_in) = match t_pub
            .exchange_code_for_tokens(&payload.code)
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => { /* ... error handling unchanged ... */ }
        };

        // Exchange the short-lived token for a long-lived one
        let (long_lived_token, long_expires_in) = match t_pub
            .exchange_for_long_lived_token(&access_token)
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                tracing::warn!("Failed to exchange for long-lived Threads token: {}", e);
                // Fall back to short-lived token
                (access_token.clone(), 3600u64)
            }
        };

        let final_user_id = user_id.or_else(|| {
            tracing::warn!("Threads token exchange did not return user_id");
            None
        });

        let updated = PublisherConfig::Threads {
            client_id: t_pub.client_id.clone(),
            client_secret: t_pub.client_secret.clone(),
            access_token: Some(long_lived_token),
            user_id: final_user_id,
            redirect_uri: Some(t_pub.redirect_uri.clone()),
            template: t_pub.template.clone(),
        };

        if let Err(e) = state.db.upsert_publisher(&id, &updated, true).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Failed to save tokens: {e}") })),
            )
                .into_response();
        }

        return Json(json!({ "ok": true, "message": "OAuth completed for Threads" }))
            .into_response();
    }
    ```

- [ ] **Step 2: Do the same in `callback_get` function for Threads**

    In the `callback_get` function, the `"threads"` match arm (around line 803) also exchanges tokens. Add the long-lived exchange there too, in the same pattern:

    ```rust
    "threads" => {
        // ... downcast and initial exchange unchanged ...

        let (access_token, user_id, _expires_in) =
            match t_pub.exchange_code_for_tokens(&code).await { /* ... */ };

        // Exchange for long-lived token
        let (long_lived_token, _) = match t_pub
            .exchange_for_long_lived_token(&access_token)
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                tracing::warn!("Failed to exchange for long-lived Threads token: {}", e);
                access_token.clone()
            }
        };

        let final_user_id = user_id.or_else(|| {
            tracing::warn!("Threads token exchange did not return user_id");
            None
        });

        let updated = PublisherConfig::Threads {
            client_id: t_pub.client_id.clone(),
            client_secret: t_pub.client_secret.clone(),
            access_token: Some(long_lived_token),
            user_id: final_user_id,
            redirect_uri: Some(t_pub.redirect_uri.clone()),
            template: t_pub.template.clone(),
        };

        // ... upsert and response unchanged ...
    }
    ```

- [ ] **Step 3: Make sure `callback_get` thread publisher creation also gets DB reference**

    Line 585 in `callback_get`:
    ```rust
    let publisher = match create_publisher(publisher_id.clone(), &config) {
    ```
    Should become:
    ```rust
    let publisher = match create_publisher_with_config_path(publisher_id.clone(), &config, None, Some(state.db.clone())) {
    ```

- [ ] **Step 4: Build to verify**

    ```bash
    cd /data/rust/populatrs/backend && cargo build 2>&1 | head -30
    ```

    Expected: compiles successfully.

---

### Task 7: Run tests and lints

- [ ] **Step 1: Run `cargo test`**

    ```bash
    cd /data/rust/populatrs/backend && cargo test 2>&1
    ```

    Expected: all tests pass.

- [ ] **Step 2: Run `cargo clippy -- -D warnings`**

    ```bash
    cd /data/rust/populatrs/backend && cargo clippy -- -D warnings 2>&1
    ```

    Expected: no warnings.

- [ ] **Step 3: Run `cargo fmt -- --check`**

    ```bash
    cd /data/rust/populatrs/backend && cargo fmt -- --check 2>&1
    ```

    Expected: no formatting differences.

- [ ] **Step 4: Fix any issues and re-run until all pass**

    If clippy or fmt fail, fix the reported issues and re-run until all three checks pass.

---

### Summary of all files modified

| File | Changes |
|---|---|
| `backend/src/publisher/x.rs` | Add `db: Option<Arc<Database>>` field, add `db` param to `new()`, implement `save_tokens_to_config` to call `db.upsert_publisher()` |
| `backend/src/publisher/threads.rs` | Add `db: Option<Arc<Database>>` field, wrap `access_token` and `token_expires_at` in `Arc<Mutex<>>`, add `exchange_for_long_lived_token()`, `get_valid_access_token()`, `save_tokens_to_config()`; update `publish()` to use them; fix UTF-8 truncation |
| `backend/src/publisher/mod.rs` | Add `db` param to `create_publisher_with_config_path`, pass to `XPublisher::new()` and `ThreadsPublisher::new()`, add `db` field to `PublisherManager`, update `add_publisher` to pass DB through, add `new_with_db` constructor |
| `backend/src/main.rs` | In scheduler loop, create `PublisherManager` with `new_with_db(None, Some(Arc::new(db.clone())))` |
| `backend/src/routes/oauth.rs` | Pass `Some(state.db.clone())` to publisher creation in `authorize`, `callback`, and `callback_get`; add long-lived token exchange in Threads callback sections |