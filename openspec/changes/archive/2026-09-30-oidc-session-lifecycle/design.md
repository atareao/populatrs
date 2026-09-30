# Design

## Context

See `proposal.md` — Why. The current flow stores the access token in both
`sessionStorage` and `localStorage`, never inspects the JWT `exp`, and refreshes
indefinitely against a refresh token whose `expires_at` is written but never read.
Logout is client-only. The backend already has OIDC discovery, PKCE, and a
`refresh_tokens` table, so the fix builds on existing primitives rather than adding new
infrastructure.

## Goals / Non-Goals

**Goals:**
- Make login deterministic: no valid session ⇒ always visit the provider.
- Make sessions end: browser close and explicit logout both terminate access.
- Enforce refresh-token expiry server-side.
- Keep the change small and testable with the existing Rust + Vitest suites.

**Non-Goals:**
- Introducing server-side HTTP sessions or cookies (the app stays bearer-token based).
- Changing the PKCE/state CSRF mechanism.
- Adding a "remember me" / persistent-login option.
- Multi-device session management or a session list UI.

## Decisions

### D1 — Store the access token in `sessionStorage` only

`setToken` writes to `sessionStorage`; `clearToken` still removes from both stores to
purge legacy `localStorage` entries. `getToken` reads `sessionStorage` first, then
`localStorage` (for one-time cleanup of pre-existing tokens).

- **Why**: `sessionStorage` is cleared when the tab/browser closes, which is exactly the
  "session ends" semantics requested. It is a native primitive — no expiry bookkeeping.
- **Alternatives**: (a) keep `localStorage` and add an expiry check — still persists
  across restarts and requires manual cleanup; (b) cookies with `Max-Age` — larger change,
  CSRF implications. Rejected.

### D2 — Client-side JWT `exp` check with clock skew, separate from raw token access

Add `isTokenValid(token)` and `hasValidSession()` in `store/auth.ts`: split the JWT,
base64url-decode the payload, read `exp`, and compare against `Date.now()/1000` minus a
30-second skew. Any decode failure ⇒ invalid. `getToken()` keeps returning the **raw**
stored token (possibly expired); `hasValidSession()` combines presence + validity and is
what `LoginPage` uses to decide the redirect.

- **Why**: `LoginPage` decides whether to auto-redirect *before* any API call, so a
  server 401 cannot prevent the stale-token short-circuit. Decoding `exp` locally is the
  only way to gate that decision. The raw token must stay retrievable because the silent
  refresh flow sends the expired token to `/auth/refresh`, which validates the signature
  while ignoring expiration — filtering it out of `getToken()` would break refresh.
- **Alternatives**: (a) call `/api/me` on the login page to test the token — adds a round
  trip and still races with the redirect; (b) make `getToken()` return `null` for expired
  tokens — breaks the refresh flow. Rejected.

### D3 — `POST /auth/logout` returns the provider end-session URL

New public route (registered alongside `/auth/refresh`) that extracts the bearer token
manually and validates it with `validate_token_ignore_expiry`, so logout works even when
the access token has expired. It deletes the caller's refresh token from SQLite and
returns `{ "end_session_url": Option<String> }`. The URL is built from the discovered
`end_session_endpoint` plus `client_id` and `post_logout_redirect_uri` (the origin of
`OIDC_REDIRECT_URL` + `/login`). The frontend calls it, clears local tokens, then
redirects to `end_session_url` if present, otherwise to `/login`.

- **Why**: Logout must revoke server state (refresh token) and, when possible, end the
  provider SSO session. Returning the URL keeps the redirect decision in the client,
  which must clear storage first. Tolerating an expired access token ensures logout never
  fails and leaves a live refresh token behind.
- **Alternatives**: (a) `GET /auth/logout` that 302s directly — cannot clear client
  storage before leaving the origin; (b) put it behind `require_auth` — an expired access
  token would 401 and the refresh token would survive; (c) skip provider logout — leaves
  the SSO session alive, which is the reported "never ends" symptom. Rejected.
- **Note**: We do not store the `id_token`, so no `id_token_hint` is sent. PocketID
  accepts `client_id` + `post_logout_redirect_uri`; if a provider requires the hint, the
  fallback is local-only logout.

### D4 — Enforce refresh-token expiry in storage

`get_refresh_token` selects `refresh_token, expires_at`, parses `expires_at` as RFC3339,
and if it is in the past deletes the row and returns `None`. The refresh handler already
maps `None` to HTTP 401.

- **Why**: The column exists but is ignored, so refresh works forever. Comparing in Rust
  avoids SQLite string-comparison pitfalls with timezone-offset timestamps.
- **Alternatives**: SQL `WHERE expires_at > datetime('now')` — stored values are RFC3339
  with offsets, so lexicographic comparison is unreliable. Rejected.

### D5 — Parse `end_session_endpoint` in discovery

Add `end_session_endpoint: Option<String>` to `OidcMetadata`. Missing field ⇒ `None`,
logout falls back to local clearing.

## Risks / Trade-offs

- **[Users lose cross-restart persistence]** → Intended behavior; the reported bug is the
  opposite. Documented as a deliberate trade-off.
- **[Clock skew causes premature expiry]** → 30-second skew allowance; refresh flow
  recovers transparently on the next 401.
- **[Provider requires `id_token_hint` for logout]** → We fall back to local-only logout;
  the refresh token is still revoked server-side, so the app session ends even if the
  provider SSO session does not.
- **[Legacy `localStorage` tokens]** → `clearToken` removes both stores and `getToken`
  treats expired tokens as absent, so stale entries self-heal on first load.
- **[Concurrent refresh + logout]** → Logout deletes the refresh token; an in-flight
  refresh may still succeed once. Acceptable; the next request fails and the client
  redirects to login.

## Migration Plan

1. Deploy backend first (adds `/auth/logout`, expiry enforcement, discovery field). Old
   clients keep working; expired refresh tokens now correctly fail.
2. Deploy frontend (sessionStorage-only, expiry check, logout call).
3. No database migration required — `refresh_tokens.expires_at` already exists.
4. Rollback: revert both deploys; no schema change to undo.
