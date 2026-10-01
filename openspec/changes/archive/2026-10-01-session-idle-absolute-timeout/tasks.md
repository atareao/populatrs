# Tasks

## 1. Config — RED (failing tests)

- [x] 1.1 Add a Rust test in `backend/src/config.rs` asserting `SessionTimeouts` defaults to 1800 idle, 14400 absolute and 60 activity-throttle seconds when the env vars are unset; verify it fails with `cargo test config`
- [x] 1.2 Add a Rust test asserting the env vars `SESSION_IDLE_TIMEOUT_SECONDS`, `SESSION_ABSOLUTE_TIMEOUT_SECONDS` and `SESSION_ACTIVITY_THROTTLE_SECONDS` override the defaults and that an unparseable value falls back to the default; verify it fails with `cargo test config`

## 2. Config — GREEN (minimal implementation)

- [x] 2.1 Add the timeout fields (or a `SessionTimeouts` struct) to `Config` in `backend/src/config.rs`, loaded from the three env vars with the defaults above; verify 1.1 and 1.2 pass with `cargo test config`

## 3. Backend storage — RED (failing tests)

- [x] 3.1 Add tests in `backend/src/db.rs` for session creation at login (`created_at` = now, `last_activity_at` = now) and for reading the session with both timestamps; verify they fail with `cargo test`
- [x] 3.2 Add tests for `touch_activity`: it updates `last_activity_at` to now and does not change `created_at`; verify they fail with `cargo test`
- [x] 3.3 Add tests for the timeout predicate: a session is expired when `now > created_at + absolute` OR `now > last_activity_at + idle`; verify they fail with `cargo test`
- [x] 3.4 Add a test that `get_refresh_token` still returns `None` and deletes the row for a session past its provider `expires_at` (existing behavior preserved); verify it fails or is updated with `cargo test`

## 4. Backend storage — GREEN (minimal implementation)

- [x] 4.1 In `run_migrations` (`backend/src/db.rs`) add `last_activity_at` to `refresh_tokens` and defensively add `created_at` for databases that predate it, backfilling both to `datetime('now')` using the existing error-tolerant `ALTER TABLE` style; verify the app opens a fresh and a legacy database with `cargo test`
- [x] 4.2 Add `create_session`, a session lookup returning both timestamps, `touch_activity`, and the timeout predicate in `backend/src/db.rs`; verify tasks 3.1–3.4 pass with `cargo test`

## 5. Backend enforcement — RED (failing tests)

- [x] 5.1 Add tests for `require_auth` (`backend/src/middleware.rs`) covering: no session row → 401; idle-expired → 401; absolute-expired → 401; valid session → request proceeds; verify they fail with `cargo test`
- [x] 5.2 Add a test that `require_auth` does NOT update the last-activity time (a session near its idle deadline stays expired after a request); verify it fails with `cargo test`
- [x] 5.3 Add tests for the activity handler (`backend/src/routes/auth_routes.rs`): a valid session gets `204` and an updated timestamp; an idle- or absolute-expired session gets `401`; verify they fail with `cargo test`
- [x] 5.4 Add tests for `refresh_token` covering: within limits → new access token; idle-expired → 401 and session revoked; absolute-expired (even with recent activity) → 401 and session revoked; a successful refresh does not change `created_at`; verify they fail with `cargo test`

## 6. Backend enforcement — GREEN (minimal implementation)

- [x] 6.1 In `require_auth` load the session by `user_id`, revoke and return 401 on a missing or timed-out session, and do not touch activity; verify 5.1 and 5.2 pass
- [x] 6.2 Add `POST /auth/activity` in `backend/src/routes/auth_routes.rs`, registered in `backend/src/routes/mod.rs`, that updates activity for a valid session and returns 204, or 401 when timed out; verify 5.3 passes
- [x] 6.3 In `refresh_token` check the idle and absolute limits before exchanging, revoke and return 401 on timeout, and leave `created_at` untouched on success; verify 5.4 passes
- [x] 6.4 Run `cargo test` in `backend/` and confirm all tests pass

## 7. Frontend — RED (failing tests)

- [x] 7.1 Add Vitest tests in `frontend/src/test/store.auth.test.ts` that `hasValidSession()` / the validity check reports invalid when the locally-tracked last activity is older than the idle timeout, and when the locally-tracked start is older than the absolute lifetime; verify they fail with `pnpm test`
- [x] 7.2 Add Vitest tests for the session-window storage helpers (record session start at login, update last activity, read both) in `frontend/src/test/store.auth.test.ts`; verify they fail with `pnpm test`
- [x] 7.3 Add Vitest tests in `frontend/src/test/api.http.test.ts` that the activity call issues an authenticated `POST /auth/activity` and that a `401` from it is treated as session expiry; verify they fail with `pnpm test`
- [x] 7.4 Add Vitest tests for the activity tracker hook: it calls the activity endpoint on interaction, throttles repeated calls, ignores `visibilitychange` to hidden, and terminates the session when a limit has passed; verify they fail with `pnpm test`

## 8. Frontend — GREEN (minimal implementation)

- [x] 8.1 In `frontend/src/store/auth.ts` add session-window storage (start and last activity) and extend the validity check to consider the idle window and absolute lifetime; verify 7.1 and 7.2 pass
- [x] 8.2 In `frontend/src/api/http.ts` add the activity call to `POST /auth/activity` and route its `401` through the existing `SessionExpiredError` path; verify 7.3 passes
- [x] 8.3 Add the activity-tracker hook (interaction events, throttle, `visibilitychange`), wire it in `frontend/src/components/AppLayout.tsx`, and have it clear the session and navigate to `/login` on timeout; record the session start in the login flows (`frontend/src/pages/OAuthCallback.tsx` and the dev-login path); verify 7.4 passes
- [x] 8.4 Update `frontend/src/pages/LoginPage.tsx` so the auto-redirect uses the session-window-aware validity check; verify the existing login tests still pass with `pnpm test`
- [x] 8.5 Run `pnpm test` and `npx tsc -b` in `frontend/` and confirm all pass

## 9. Verification

- [x] 9.1 Run `cargo test`, `cargo clippy -- -D warnings` and `cargo fmt -- --check` in `backend/` and confirm all pass
- [x] 9.2 Run `pnpm test` and `npx tsc -b` in `frontend/` and confirm all pass
- [x] 9.3 Confirm no regression in the existing `auth/session-lifecycle` scenarios (all previously passing tests remain green)
- [x] 9.4 Manually verify: with a short idle timeout configured, leaving the dashboard open past the idle timeout returns to login; with a short absolute lifetime, activity does not extend the session past it

## 10. Archive

- [x] 10.1 Mark tasks complete and run `openspec archive session-idle-absolute-timeout --yes`
- [x] 10.2 Confirm `openspec/changes/session-idle-absolute-timeout/` was removed; delete it manually if it was not

## 11. Review fixes

- [x] 11.1 Backend: map a transient database error in the `POST /auth/activity` handler to `500 Internal Server Error` (was `401`), consistent with `enforce_session_timeouts`, so a momentary failure does not force a logout
- [x] 11.2 Backend: treat a parsed timeout value of `0` as invalid in `parse_seconds` and fall back to the default; add `test_session_timeouts_zero_falls_back_to_default`
- [x] 11.3 Backend: surface backfill errors in the `refresh_tokens` migration with `tracing::warn!` instead of silently discarding them
- [x] 11.4 Backend: expose `idle_seconds` and `absolute_seconds` from `GET /api/me` so the client uses the server-configured limits
- [x] 11.5 Backend: set `populatrs_session_start` and `populatrs_last_activity` in the server-rendered login HTML (`callback` and `dev_login`) so the session window is anchored at real login time
- [x] 11.6 Frontend: consume the server-provided limits from `/api/me` (`setSessionLimits`) with the client constants as fallback, so a client check cannot contradict a server-configured longer limit
- [x] 11.7 Frontend: make `hasValidSession` a pure predicate (remove the implicit window anchoring side effect); keep `ensureSessionWindow` as an explicit legacy fallback
- [x] 11.8 Frontend: remove the misplaced `ensureSessionWindow()` call from the publisher `OAuthCallback` page
- [x] 11.9 Frontend: remove the redundant `hiddenRef`, using `document.visibilityState` as the single source of truth
- [x] 11.10 Tests: add coverage for server-limit overrides, a runtime `visibilitychange` toggle, and `SessionExpiredError` → `/login` navigation

## Verification

- Backend: `cargo test` (114 lib + 12 main passed), `cargo clippy -- -D warnings` clean, `cargo fmt -- --check` clean.
- Frontend: `pnpm test` (165 passed), `npx tsc -b` clean.
- Server smoke test (dev mode): `GET /api/me` returns the configured `idle_seconds` / `absolute_seconds`; the login HTML sets the session-window keys; `POST /auth/activity` returns `204`.
