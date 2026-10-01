# Proposal

## Why

Sessions have no upper bound on how long they stay authenticated. The only limits today are
PocketID's 1-hour access token (silently refreshed by the client) and a 30-day sliding
refresh token in SQLite. There is no idle timeout and no absolute session lifetime, so an
open tab stays logged in for up to 30 days; a user returning after several hours is never
asked to authenticate. An admin panel should require re-authentication after a bounded
period of inactivity and after an absolute session lifetime.

## What Changes

- Enforce a **server-authoritative idle timeout of 30 minutes** (default): no authenticated
  request succeeds once 30 minutes have passed since the last genuine user activity.
- Enforce a **server-authoritative absolute session lifetime of 4 hours** (default),
  anchored at login and never extended by refresh or activity.
- Count **only explicit user activity** toward the idle window. Background polling (the
  Dashboard's 30-second status poll) MUST NOT keep the session alive.
- Add configurable environment variables `SESSION_IDLE_TIMEOUT_SECONDS` (default `1800`)
  and `SESSION_ABSOLUTE_TIMEOUT_SECONDS` (default `14400`).
- On either timeout the server revokes the stored session (deletes the refresh token) and
  responds `401`; the client clears local tokens and returns to the login page.
- The client ends the local session proactively when it can determine a timeout has passed,
  instead of waiting for a background request to fail.
- **BREAKING (internal)**: a session may now be rejected while its access token is still
  cryptographically valid, if the idle timeout or the absolute lifetime has passed.
- No "remember me" / long-lived session mode is added.

## Capabilities

### New Capabilities
<!-- None -->

### Modified Capabilities

- `auth/session-lifecycle`: add idle and absolute session timeouts, server-authoritative
  enforcement, an explicit activity signal, client-side proactive expiry, and refresh
  behavior under both limits.

## Impact

- **Backend**: `backend/src/config.rs` (new env vars), `backend/src/db.rs` (session columns,
  lookups/updates, migration), `backend/src/middleware.rs` (enforcement), 
  `backend/src/routes/auth_routes.rs` (activity endpoint, refresh checks), 
  `backend/src/routes/mod.rs` (route registration).
- **Frontend**: `frontend/src/store/auth.ts` (session timestamps and validity), 
  `frontend/src/api/http.ts` (activity call and expiry handling), a new activity hook,
  `frontend/src/components/AppLayout.tsx` (wire the tracker), 
  `frontend/src/pages/LoginPage.tsx` (valid-session check).
- **APIs**: new `POST /auth/activity`; `POST /auth/refresh` may return `401` on idle or
  absolute timeout; protected endpoints return `401` after a timeout.
- **Config**: two new environment variables with defaults.
- **Tests**: Rust unit/integration tests and Vitest tests for the tracker and validity.
