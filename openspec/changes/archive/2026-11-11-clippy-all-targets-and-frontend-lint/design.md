# Design

## Context
- `backend/src/routes/auth_routes.rs` has one pre-existing `clippy::match_like_matches_macro`
  violation at line 867, inside a `#[cfg(test)]` module. `cargo clippy -- -D warnings`
  (what CI runs today) does not see test targets, so it is invisible in CI but breaks any
  local run of `cargo clippy --all-targets -- -D warnings`.
- `frontend/package.json` defines `dev`, `build`, `preview`, `test` and `test:watch`, but no
  `lint` script, and the repository ships no ESLint configuration.
- `.github/workflows/ci.yml` runs only backend `cargo test`, `cargo fmt --check` and clippy
  without `--all-targets`; there is no frontend job.
- Baseline measured on the current `development` branch: `cargo clippy --all-targets` fails
  with exactly one error; `npx vitest run` reports 13 files / 165 tests passing; `npx tsc -b`
  succeeds; the frontend lockfile is `pnpm-lock.yaml` with `lockfileVersion: '9.0'`.

## Goals / Non-Goals

**Goals:**
- Make `cargo clippy --all-targets -- -D warnings` green locally and in CI.
- Give the frontend a working `lint` script backed by a real ESLint configuration, green on
  the current sources.
- Have CI enforce the same gates developers run locally, for both stacks.

**Non-Goals:**
- Introducing Prettier, formatting enforcement in CI for JS/TS, or re-formatting frontend
  sources.
- Changing any production behavior, API contract or database schema.
- Fixing the unrelated release-automation problems tracked in `HANDOFF.md` (broken
  `release-prepare.yml` token, `git-cliff` changelog section).

## Decisions

1. **Rewrite the lint violation instead of silencing it.** clippy's own suggestion is
   applied verbatim:
   `let is_mismatch = !matches!((&cb_state, &stored), (Some(cb), Some((ref stored_state, _, _))) if stored_state == cb);`
   Rationale: `#[allow(...)]` would hide the class of issue and contradicts `AGENTS.md`'s
   "zero warnings" rule. The expression lives in a test asserting state-mismatch detection
   and keeps identical semantics.
   Alternative considered: keeping `match` and adding `#[allow(clippy::match_like_matches_macro)]`
   — rejected.

2. **ESLint 9 with flat config and type-aware-free typescript-eslint.** New
   `frontend/eslint.config.js` using `@eslint/js`, `typescript-eslint` (recommended, no
   type-aware parsing), `eslint-plugin-react-hooks` and `eslint-plugin-react-refresh`, with
   `dist/` and `node_modules/` ignored and browser globals for the app. Script:
   "lint": "eslint .".
   Rationale: matches the Vite + React 19 template the frontend is built from, keeps the
   dependency surface small and the first run fast, and avoids a large initial violation
   count that type-aware rules would produce.
   Alternative considered: Airbnb-style / `eslint-config-prettier` presets — rejected as
   unnecessary churn and scope.

3. **Pin the frontend toolchain in CI via `packageManager`.** Add
   "packageManager": "pnpm@<local-major>" to `frontend/package.json` and let
   `pnpm/action-setup@v4` read it, or pass the same version explicitly. Node is pinned to the
   version already used locally.
   Rationale: prevents CI from installing a pnpm major that rewrites or rejects
   `lockfileVersion: '9.0'`.

4. **Add a separate `frontend` CI job alongside the existing backend job.** The existing
   `test` job keeps clippy with `--all-targets` added; the new job installs pnpm, runs
   `pnpm install --frozen-lockfile`, then `pnpm lint`, `npx tsc -b` and `pnpm test`.
   Rationale: parallel jobs give clearer failure attribution and avoid re-running backend
   work when only the frontend changed.

## Risks / Trade-offs

- **First ESLint run may surface violations.** Any violations found while making the gate
  green must be fixed in `src/`, or narrowly and explicitly disabled with a comment
  justifying it; blanket rule disabling is not acceptable.
- **New devDependency footprint** (`eslint`, `@eslint/js`, `typescript-eslint`, plugins)
  grows the lockfile and CI install time. Accepted as the cost of an enforceable frontend gate.
- **CI runtime increases** by one frontend job (dependency install + lint + typecheck +
  tests). Accepted; jobs run in parallel.
- **Version pinning drift**: if the pinned pnpm version is unavailable on the runner, the job
  fails fast and the version must be bumped deliberately — this is the intended, explicit
  failure mode.