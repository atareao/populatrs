# Proposal

## Why

`docs/oidc-token-refresh.md` describes behaviour that predates the current implementation: it claims the access token is stored in `localStorage` and survives a browser restart, and it frames the 30-day provider refresh token as the effective session lifetime. Since the `session-idle-absolute-timeout` change, Populatrs enforces a server-authoritative 30-minute idle timeout and a 4-hour absolute session lifetime, and the token is `sessionStorage`-only. The guide now misleads its readers.

## What Changes

- Correct the token-storage description to `sessionStorage`-only (no `localStorage`).
- Replace the "30-day session" framing with the idle (30 min) + absolute (4 h) policy.
- Document the explicit activity signal (`POST /auth/activity`), server-authoritative enforcement, and the configurable environment variables.
- Mark the implementation checklist as done and fix the "Flujo deseado" snippet.

Documentation-only change; no runtime behaviour changes, hence `skip_specs: true`.

## Capabilities

### New Capabilities
<!-- None -->

### Modified Capabilities
<!-- None: documentation-only, no spec-level behaviour change. -->

## Impact

- `docs/oidc-token-refresh.md` only.
- No code, API, config, or spec changes.
