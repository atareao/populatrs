# Tasks

## 1. Client — RED (failing tests)

- [x] 1.1 Add a Vitest test in `frontend/src/test/store.auth.test.ts` that `clearToken()` increments the session generation exposed by `getSessionGeneration()`; verify it fails with `pnpm test`
- [x] 1.2 Add a Vitest test in `frontend/src/test/api.http.test.ts` that a silent refresh resolving *after* `clearToken()` does not write a token to `sessionStorage` and rejects with `SessionExpiredError`; verify it fails with `pnpm test`
- [x] 1.3 Add a Vitest test that a silent refresh resolving *before* `clearToken()` stores the new token normally; verify it fails with `pnpm test`

## 2. Client — GREEN (minimal implementation)

- [x] 2.1 Add the `sessionGeneration` counter and `getSessionGeneration()` to `frontend/src/store/auth.ts`; increment it in `clearToken()`; verify 1.1 passes
- [x] 2.2 In `frontend/src/api/http.ts`, capture the generation in `refreshAccessToken()` before the request and skip `setToken()` (throwing `SessionExpiredError`) when it changed; verify 1.2 and 1.3 pass

## 3. Server — RED (failing tests)

- [x] 3.1 Add a test in `backend/src/db.rs` for `save_refresh_token_if_current`: it updates and returns `true` when the stored token matches, and returns `false` without inserting when the row is absent or the token differs; verify it fails with `cargo test`
- [x] 3.2 Add a test in `backend/src/routes/auth_routes.rs` covering the logout-during-refresh race at the storage level (token removed between read and save → rotation not persisted); verify it fails with `cargo test`

## 4. Server — GREEN (minimal implementation)

- [x] 4.1 Implement `Database::save_refresh_token_if_current` in `backend/src/db.rs` using `UPDATE ... WHERE user_id = ? AND refresh_token = ?` and returning whether a row changed; verify 3.1 passes
- [x] 4.2 Use `save_refresh_token_if_current` for rotation in `refresh_token` (`backend/src/routes/auth_routes.rs`), passing the token that was read; on `false`, log a warning and return `401 Unauthorized`; verify 3.2 passes

## 5. Review fixes

- [x] 5.1 Server: remove the read-then-delete from the provider-failure path — it could delete a token that a concurrent successful refresh was rotating, making the winner's CAS fail and killing a just-renewed session (regression). The CAS is the only owner of rotation; logout deletes explicitly.
- [x] 5.2 Server: always run the CAS after a successful exchange, even when the provider returns no new refresh token (re-save the same token with a renewed expiry), so a concurrent logout always rejects the request. Extracted as `persist_rotated_token`.
- [x] 5.3 Server: add tests for `persist_rotated_token` (new token present, absent, row removed, and absent + row removed).
- [x] 5.4 Server: map a database failure during rotation to `500 Internal Server Error` (transient, retryable) instead of `401`; only a failed CAS (`false`) returns `401`.

## 6. Verification

- [x] 6.1 Run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check` in `backend/` and confirm all pass
- [x] 6.2 Run `pnpm test` and `npx tsc -b` in `frontend/` and confirm all pass
- [x] 6.3 Confirm no regression in the existing `auth/session-lifecycle` scenarios (all previously passing tests remain green)

## 7. Archive

- [x] 7.1 Mark tasks complete and run `openspec archive oidc-logout-refresh-race --yes`
- [x] 7.2 Confirm `openspec/changes/oidc-logout-refresh-race/` was removed; delete manually if not
