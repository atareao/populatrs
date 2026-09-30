# Tasks

## 1. Backend — RED (failing tests)

- [x] 1.1 Add unit tests in `backend/src/routes/auth_routes.rs` for `logout` (deletes refresh token, returns `end_session_url` when discovery has endpoint, `null` otherwise) and verify they fail with `cargo test`
- [x] 1.2 Add unit tests in `backend/src/db.rs` for `get_refresh_token` returning `None` and deleting the row when `expires_at` is in the past, and returning the token when in the future; verify they fail with `cargo test`
- [x] 1.3 Add a unit test for `OidcMetadata` deserialization with and without `end_session_endpoint`; verify it fails with `cargo test`

## 2. Backend — GREEN (minimal implementation)

- [x] 2.1 Add `end_session_endpoint: Option<String>` to `OidcMetadata` in `backend/src/auth.rs`; verify 1.3 passes
- [x] 2.2 Update `get_refresh_token` in `backend/src/db.rs` to parse `expires_at`, delete expired rows, and return `None`; verify 1.2 passes
- [x] 2.3 Implement `logout` handler in `backend/src/routes/auth_routes.rs` (public route: manual bearer extraction + `validate_token_ignore_expiry`, delete refresh token for the user, build `end_session_url` from discovery + `client_id` + `post_logout_redirect_uri`); verify 1.1 passes
- [x] 2.4 Register `POST /auth/logout` in `backend/src/routes/mod.rs` (public router, alongside `/auth/refresh`); verify route resolves via `cargo check`
- [x] 2.5 Verify `POST /auth/refresh` returns 401 for an expired stored refresh token (covered by 1.2 + existing handler); run `cargo test`

## 3. Frontend — RED (failing tests)

- [x] 3.1 Add Vitest tests for `isTokenValid` / `hasValidSession` / `getToken` in `frontend/src/store/auth.test.ts` (valid, expired, malformed, missing; raw token still returned when expired) and verify they fail with `pnpm test`
- [x] 3.2 Add a Vitest test for `LoginPage` that an expired stored token does not auto-redirect and is cleared, and that a valid token redirects; verify it fails with `pnpm test`
- [x] 3.3 Add a Vitest test for `AppLayout` logout calling `POST /auth/logout` and clearing tokens; verify it fails with `pnpm test`

## 4. Frontend — GREEN (minimal implementation)

- [x] 4.1 Implement `isTokenValid` and `hasValidSession`, and update `getToken`/`setToken`/`clearToken` in `frontend/src/store/auth.ts` (sessionStorage-only writes, raw `getToken`, expiry-aware `hasValidSession`); verify 3.1 passes
- [x] 4.2 Update `frontend/src/pages/LoginPage.tsx` to clear stale tokens and only auto-redirect on a valid token; verify 3.2 passes
- [x] 4.3 Add `logout()` to `frontend/src/api/http.ts` and call it from `AppLayout.handleLogout`, redirecting to `end_session_url` or `/login`; verify 3.3 passes
- [x] 4.4 Ensure `refreshAccessToken` in `frontend/src/api/http.ts` stores the new token via `setToken` (sessionStorage only); verify no `localStorage` write remains

## 6. Review fixes

- [x] 6.1 Backend: `#[instrument(skip(state, headers))]` on `logout` (and `refresh_token`) so the bearer token is not logged; verify `cargo test` + `cargo clippy -- -D warnings`
- [x] 6.2 Backend: `build_end_session_url` fails closed — return `None` on parse failure and reject non-`http(s)` schemes; verify new unit tests pass
- [x] 6.3 Backend: omit `post_logout_redirect_uri` when it cannot be derived instead of passing the callback URL; verify unit test
- [x] 6.4 Backend: add handler-level `logout` tests (valid token deletes row + URL; expired-but-signed token still deletes; no endpoint → null; missing/invalid bearer → 401); verify `cargo test`
- [x] 6.5 Frontend: `useAuth` exposes an `error` state; `ProtectedRoute` renders a retry screen on transient failure instead of redirecting (breaks the loop); verify new test
- [x] 6.6 Frontend: validate `end_session_url` scheme (`http`/`https`) and treat empty string as absent before navigating; verify new test
- [x] 6.7 Frontend: add `aria-label` to the collapsed logout and collapse-toggle buttons; verify test/tsc
- [x] 6.8 Frontend: `useAuth` imports `getToken` from `store/auth` (remove duplicate); remove the dead `?token=` branch in `LoginPage`; guard double-click on logout; verify `pnpm test` + `npx tsc -b`
- [x] 6.9 Frontend: tighten `decodeJwtPayload` to require exactly 3 segments; add clock-skew boundary tests; verify `pnpm test`

## 7. Verification

- [x] 7.1 Run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check` in `backend/` and confirm all pass
- [x] 7.2 Run `pnpm test` and `npx tsc -b` in `frontend/` and confirm all pass
- [x] 7.3 Manual smoke test: expired token in storage shows login button (no bypass); logout revokes refresh token and redirects to provider end-session when configured

## 8. Archive

- [x] 8.1 Mark tasks complete and run `openspec archive oidc-session-lifecycle --yes`
- [x] 8.2 Confirm `openspec/changes/oidc-session-lifecycle/` was removed; delete manually if not
