# Proposal

## Why

Two related defects make the OIDC login flow unreliable and the session unending:

1. **Login bypass.** Clicking "Iniciar con OIDC" sometimes lands directly in the app
   without visiting PocketID. `LoginPage` auto-redirects whenever *any* token exists in
   storage, and `getToken()` reads `localStorage` without checking the JWT `exp`. A stale
   token left in `localStorage` (written by the silent refresh flow) short-circuits login.
2. **Session never ends.** Tokens are persisted in `localStorage` (survives browser
   close), the client never checks expiry, the backend refreshes tokens indefinitely
   (`validate_token_ignore_expiry` + a hardcoded 30-day refresh token that is never
   checked against `expires_at`), and logout only clears client storage — it neither
   revokes the server-side refresh token nor ends the PocketID SSO session.

## What Changes

- **Client token storage**: stop writing the access token to `localStorage`; use
  `sessionStorage` only, so closing the browser ends the session.
- **Client expiry check**: `getToken()` decodes the JWT `exp` and returns `null` for
  expired tokens (with a small clock-skew allowance). `LoginPage` only auto-redirects on
  a *valid* token; otherwise it clears the stale token and shows the login button.
- **Server-side logout**: new `POST /auth/logout` endpoint that deletes the user's
  refresh token from SQLite and returns the OIDC `end_session_endpoint` (RP-initiated
  logout) when available. `AppLayout.handleLogout` calls it before clearing local state.
- **Refresh token expiry enforcement**: `get_refresh_token` ignores/deletes rows whose
  `expires_at` has passed; the refresh handler rejects expired refresh tokens.
- **OIDC discovery**: parse `end_session_endpoint` from the discovery document.
- **BREAKING (internal)**: `setToken` no longer mirrors to `localStorage`; any code
  relying on cross-browser-restart persistence must be updated.

## Capabilities

### New Capabilities
- `auth/session-lifecycle`: OIDC login initiation, callback token handling, client-side
  token validity, silent refresh, and explicit logout/session termination.

### Modified Capabilities
<!-- None: no existing spec covers authentication. -->

## Impact

- **Frontend**: `src/store/auth.ts`, `src/hooks/useAuth.ts`, `src/pages/LoginPage.tsx`,
  `src/components/AppLayout.tsx`, `src/api/http.ts`.
- **Backend**: `src/auth.rs` (`OidcMetadata`, `JwtValidator`), `src/routes/auth_routes.rs`
  (new `logout`, refresh expiry), `src/routes/mod.rs` (route), `src/db.rs`
  (`get_refresh_token` expiry filter).
- **APIs**: new `POST /auth/logout`; `POST /auth/refresh` now rejects expired refresh
  tokens.
- **Config**: no new env vars; uses existing `OIDC_ISSUER_URL` / discovery.
- **Tests**: new Rust unit tests for logout/expiry; new Vitest tests for token expiry
  and login redirect behavior.
