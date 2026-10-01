# Design

## Context

See `proposal.md` — Why. Relevant current state:

- The client stores the access token in `sessionStorage` (`frontend/src/store/auth.ts`) and
  silently refreshes it on `401` (`frontend/src/api/http.ts`).
- The server stores one refresh token per user in the `refresh_tokens` table
  (`backend/src/db.rs`). The table already declares `user_id`, `refresh_token`, `expires_at`
  and `created_at TEXT NOT NULL DEFAULT (datetime('now'))`; `created_at` is currently unused.
  Existing databases created before that column existed do not have it, because
  `CREATE TABLE IF NOT EXISTS` does not add columns.
- `require_auth` (`backend/src/middleware.rs`) validates the JWT and inserts an `AuthUser`.
  It has access to `Arc<AppState>` (and therefore the database) via request extensions.
- `refresh_token` (`backend/src/routes/auth_routes.rs`) validates the JWT ignoring expiry,
  reads the stored refresh token, exchanges it with PocketID, and rotates it with a
  compare-and-swap.
- `Dashboard.tsx` polls `GET /api/status` every 30 seconds via `setInterval`. Any design
  that treats an arbitrary authenticated request as user activity would let this poll keep
  the session alive forever.
- PocketID access tokens are hardcoded to 1 hour and are not configurable per client, so a
  shorter access token is not available to us.

## Goals / Non-Goals

**Goals:**

- Enforce a bounded idle window (default 30 min) and an absolute session lifetime
  (default 4 h) authoritatively on the server, independent of the access token's `exp`.
- Keep background polling from counting as user activity.
- Fail closed: when a limit is exceeded, revoke the session and require login.
- Make both limits configurable via environment variables with sane defaults.
- Keep the change local to the existing auth modules; no new runtime dependencies.

**Non-Goals:**

- Multi-device or per-device sessions. The existing model is one session per user (one row
  keyed by `user_id`); this change preserves that.
- "Remember me" or any long-lived session mode.
- Shortening the PocketID access token lifetime (not configurable).
- Moving the access token from `sessionStorage` to an `HttpOnly` cookie (tracked as a
  possible follow-up, out of scope here).

## Decisions

### D1 — Session state lives on the existing `refresh_tokens` row

Reuse the per-user `refresh_tokens` row as the session record. Treat its `created_at` as the
immutable absolute-lifetime anchor (set at login, never reset), and add a `last_activity_at`
column updated only by genuine user activity. Keep `expires_at` as the provider refresh
token validity.

- **Why**: one row per user already exists and is already the session's server-side handle;
  adding one column is a minimal, low-risk migration. `created_at` already exists on fresh
  databases and can be reused as the anchor.
- **Alternatives considered**:
  - A separate `sessions` table keyed by a random session id: more correct for multi-device,
    but it requires a new session identifier threaded through every request (the JWT has no
    such claim) and a larger refactor. Rejected as out of scope.
  - An in-memory map of sessions in `AppState`: lost on restart, and inconsistent with the
    existing SQLite-persisted session.

### D2 — Enforcement point is `require_auth`

After validating the JWT, `require_auth` loads the session by `user_id`. If no session
exists, or the idle window or absolute lifetime has passed, it revokes the session (deletes
the row) and returns `401`. Ordinary authenticated requests do NOT update
`last_activity_at`, so background polling cannot extend the session.

- **Why**: a single choke point covers every protected route and cannot be forgotten in a
  new handler.
- **Alternatives considered**:
  - Per-handler checks: error-prone and repetitive.
  - Updating activity on every request with a denylist of polling endpoints: brittle; the
    denylist would silently rot as endpoints change.

### D3 — Activity is an explicit, authenticated signal

Add `POST /auth/activity`, which updates `last_activity_at = now` (subject to the same
limits) and returns `204`. The client calls it on genuine user interaction, throttled to at
most once per `SESSION_ACTIVITY_THROTTLE_SECONDS` (default `60`). Background polling never
calls it.

- **Why**: explicitly distinguishes user activity from machine traffic, which is the only
  way the 30-second status poll can be allowed to run without defeating the idle timeout.
- **Alternatives considered**:
  - A custom header on every user-initiated request: requires classifying every call site
    as user- or background-initiated; easy to get wrong.
  - Counting every request: defeats the idle timeout because of the poll.

### D4 — A successful refresh never extends the absolute lifetime

`POST /auth/refresh` checks the idle and absolute limits before exchanging; if either is
exceeded it revokes the session and returns `401`. On success it rotates the refresh token
but leaves `created_at` untouched, so the 4-hour cap holds even under continuous refreshing.
Only the activity signal moves `last_activity_at`.

- **Why**: the whole point of the absolute lifetime is that no amount of activity or
  refreshing can extend it.
- **Alternatives considered**: resetting `created_at` on refresh — rejected, that is the
  current unbounded behavior.

### D5 — Timeouts are configuration with defaults

Add `SESSION_IDLE_TIMEOUT_SECONDS` (default `1800`) and
`SESSION_ABSOLUTE_TIMEOUT_SECONDS` (default `14400`) to `Config`, plus
`SESSION_ACTIVITY_THROTTLE_SECONDS` (default `60`). Values are parsed like existing numeric
settings and fall back to the default on absence or parse failure.

- **Why**: matches the project's env-var configuration pattern and lets operators tighten
  or loosen the policy without a rebuild.

### D6 — The client ends the session proactively

The client records the session start and last activity in `sessionStorage`, runs a lightweight
activity tracker (interaction events, throttled) that calls `POST /auth/activity`, and on a
periodic check ends the local session (clears tokens, navigates to `/login`) when the idle
or absolute limit has passed. The client-side check is best-effort; the server remains
authoritative.

- **Why**: without it, the only signal of expiry is a failed background request, which can
  produce a laggy or bouncing logout. With it, the user is returned to login deterministically.
- **Alternatives considered**: relying solely on server `401`s — simpler, but the dashboard
  poll can make the logout appear up to ~30 s late and may briefly bounce between the app
  and login. The client check complements, and does not replace, server enforcement.

### D7 — Client-side validity accounts for the session window

Extend the client validity check to treat a session as invalid when the locally-tracked idle
window or absolute lifetime has passed, in addition to the existing JWT `exp` check with
clock-skew tolerance. This keeps `LoginPage` from auto-redirecting into an app whose session
has already timed out.

- **Why**: `LoginPage` currently decides based only on JWT `exp` (up to 1 hour), which would
  misclassify a session that is idle-expired after 30 minutes as still valid.

## Risks / Trade-offs

- [A user is actively typing in a long form but the app is idle at the server because the
  activity throttle delays the signal] → The throttle is small (60 s) relative to the idle
  window (30 min); interaction signals fire well within the window.
- [Client and server clocks differ when the client checks expiry proactively] → Use the
  existing clock-skew tolerance and treat the client check as best-effort; the server is
  authoritative and will reject anyway.
- [Existing sessions at deploy time lack `last_activity_at` / may lack `created_at`] →
  Migration adds any missing columns and backfills them to `datetime('now')`, which starts
  existing sessions' clock at deploy time (they will time out within 4 h at most).
- [A hidden/background tab continues to call the poll but sends no activity] → Intended: the
  idle timeout fires and the next request returns `401`. `visibilitychange` is used to avoid
  sending activity for hidden tabs.
- [SQLite write amplification from activity updates] → Throttled to at most one update per
  `SESSION_ACTIVITY_THROTTLE_SECONDS` per client, and only on genuine interaction.

## Migration Plan

1. `run_migrations` adds `last_activity_at` to `refresh_tokens` and, defensively,
   `created_at` for databases that predate it; both backfilled to `datetime('now')`. The
   `ALTER TABLE ... ADD COLUMN` calls ignore "duplicate column" errors, matching the existing
   migration style.
2. Deploy backend, then frontend. Existing sessions get a fresh anchor at migration time.
3. Rollback: revert the code; the added columns are harmless and the previous code does not
   read them.

## Open Questions

None blocking. Whether to also move the access token to an `HttpOnly` cookie is a separate
hardening item, deferred.
