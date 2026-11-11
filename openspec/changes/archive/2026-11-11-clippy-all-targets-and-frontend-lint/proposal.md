# Proposal

## Why
`AGENTS.md` prescribes `cargo clippy --all-targets -- -D warnings` and `pnpm lint` as
mandatory quality gates, but neither works today: clippy with `--all-targets` fails on a
pre-existing lint in a test target, and the frontend has no `lint` script and no ESLint
configuration at all. Consequently CI can only enforce the weaker
`cargo clippy -- -D warnings` and never checks the frontend, so local development and CI
disagree and regressions in test code or frontend sources slip through.

## What Changes
- Fix the `clippy::match_like_matches_macro` violation in the `#[cfg(test)]` module of
  `backend/src/routes/auth_routes.rs` by rewriting the `match` expression as `matches!`.
- Add ESLint (flat config) to `frontend/`, plus a `lint` package script, so that
  `pnpm lint` / `npx eslint .` works as `AGENTS.md` requires.
- Strengthen CI: run clippy with `--all-targets`, and add a frontend job that installs with
  pnpm (frozen lockfile) and runs lint, type-check and tests.
- No runtime behavior changes: the edits are tooling, configuration and a test-only rewrite.

## Capabilities

### New Capabilities
- `ci/quality-gates`: the contract that the repository's static-analysis and test gates
  (backend format, clippy and tests; frontend lint, type-check and tests) exist and exit
  successfully on a clean checkout, and that CI enforces them on every pull request.

### Modified Capabilities
(none)

## Impact
- `backend/src/routes/auth_routes.rs` — test-only change, no production code path affected.
- `frontend/package.json`, new `frontend/eslint.config.js`, `frontend/pnpm-lock.yaml`.
- `.github/workflows/ci.yml`.
- No API contract, database or runtime behavior change. No new runtime dependencies; all
  ESLint packages are devDependencies.