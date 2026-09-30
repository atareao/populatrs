# Proposal

## Why

The archived `oidc-session-lifecycle` change fixed the login bypass and made sessions
terminable, but a race between an in-flight silent refresh and logout can still resurrect
the session: the refresh writes a fresh access token to `sessionStorage` *after* logout
cleared it, so the login page sees a valid session and auto-redirects back into the app.
A symmetric race exists server-side, where a concurrent refresh can re-save a refresh
token that logout just revoked. Both races undermine the guarantee that logout ends the
session.

## What Changes

- **Client**: invalidate in-flight silent refreshes when the session is cleared, so a
  refresh that resolves after logout cannot re-store an access token.
- **Server**: make refresh-token rotation conditional on the stored token still being the
  one that was read, so a refresh cannot re-create a token revoked by a concurrent logout.
- Add tests covering both races (client and server).

## Capabilities

### Modified Capabilities

- `auth/session-lifecycle`: add requirements that logout wins over concurrent refreshes on
  both the client and the server.

## Impact

- `frontend/src/store/auth.ts`, `frontend/src/api/http.ts` and their tests.
- `backend/src/db.rs`, `backend/src/routes/auth_routes.rs` and their tests.
- No API contract change. `POST /auth/refresh` may now return HTTP 401 when the session was
  revoked concurrently, which the client already treats as a genuine session expiry.
