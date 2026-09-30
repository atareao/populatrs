# Design

## Context

See `proposal.md` — Why. The relevant current state:

- Client: `frontend/src/api/http.ts` keeps a module-level `refreshPromise` that is shared
  by concurrent 401 retries. `refreshAccessToken()` reads the raw token, calls
  `POST /auth/refresh`, and unconditionally calls `setToken()` on success.
  `clearToken()` in `frontend/src/store/auth.ts` removes the token from both stores.
- Server: `refresh_token` in `backend/src/routes/auth_routes.rs` reads the stored refresh
  token, exchanges it with the provider, then calls `save_refresh_token()` — an
  unconditional upsert. `logout` calls `delete_refresh_token()`.

Both flows are read-modify-write sequences with no coordination, so a logout interleaved
between the read and the write is lost.

## Goals / Non-Goals

**Goals:**

- Guarantee that once logout clears the session, no concurrent refresh can restore it,
  on either the client or the server.
- Keep the fix minimal and local to the existing auth modules; no new dependencies.

**Non-Goals:**

- Serializing concurrent refreshes from the same user (pre-existing, tolerated: the
  provider rotates and the loser gets 401).
- Changing the OIDC provider interaction or the `POST /auth/refresh` request/response
  shape.
- Fixing the pre-existing 30-day heuristic expiry of stored refresh tokens.

## Decisions

### D1 — Client: session generation counter

Add a module-level `sessionGeneration` counter in `frontend/src/store/auth.ts`, exported
via `getSessionGeneration()`. `clearToken()` increments it. `refreshAccessToken()` captures
the generation before the network call and, after a successful response, only calls
`setToken()` when the generation is unchanged; otherwise it throws `SessionExpiredError`
without storing anything.

- **Why**: a monotonic counter is the smallest mechanism that lets an async continuation
  detect "the session was cleared while I was waiting". It also covers logout paths that
  clear the token from anywhere, not just `AppLayout`.
- **Alternatives considered**:
  - `AbortController` per refresh: aborts the request but does not prevent a response that
    already arrived from being stored; also requires threading a signal through `fetcher`.
  - Comparing the token before/after: the refresh legitimately changes the token, so this
    cannot distinguish a rotation from a resurrection.
  - A React state/context flag: not reachable from the non-React `http.ts` module.

### D2 — Server: compare-and-swap on rotation

Add `Database::save_refresh_token_if_current(user_id, expected_token, new_token,
expires_in) -> Result<bool>` that runs
`UPDATE refresh_tokens SET refresh_token = ?, expires_at = ? WHERE user_id = ? AND
refresh_token = ?` and reports whether a row was updated. The `refresh_token` handler uses
it for rotation, passing the token it read. When it returns `false`, the handler logs a
warning and responds `401 Unauthorized` instead of returning the new access token.

- **Why**: SQLite executes the `UPDATE ... WHERE` atomically under the existing connection
  mutex, so a logout that deleted the row (or another refresh that rotated it) makes the
  update a no-op. This closes the window without a new lock or shared state.
- **Alternatives considered**:
  - Per-user async mutex in `AppState`: correct but invasive (new map, lock acquisition in
    two handlers, cleanup) for a narrow race.
  - A logout "epoch" column: more schema surface than needed; CAS already expresses the
    invariant.
  - Re-reading the token before saving: still a TOCTOU window between the read and the
    write.

### D3 — Reject rather than silently drop on CAS failure

On CAS failure the handler returns 401 rather than returning the freshly minted access
token. The client already maps a 401 from `/auth/refresh` to `SessionExpiredError`, so the
behavior is consistent with an expired session.

- **Why**: returning a token whose refresh token was not persisted would leave the client
  in a session it cannot renew.
- **Alternatives considered**: returning the access token anyway — simpler, but hides the
  revocation and can produce a half-dead session.

## Risks / Trade-offs

- [A concurrent refresh that loses the CAS now gets 401 instead of a token] → Acceptable:
  the winning refresh already delivered a valid token to the client; the loser retries with
  it.
- [The generation counter is module state, not persisted] → Intended: it only needs to
  order events within a single page lifetime, which is exactly the race window.
- [CAS failure is logged at warn level] → Could be noisy under heavy concurrent refresh;
  mitigated by the client-side `refreshPromise` coalescing, which already prevents most
  concurrent refreshes from the same tab.
