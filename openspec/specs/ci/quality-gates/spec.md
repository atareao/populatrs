# ci/quality-gates Specification

## Purpose
Defines the quality gates every contribution must pass in CI: backend formatting, linting
and tests, plus frontend linting, type-checking and tests.

## Requirements

### Requirement: Backend strict lint gate
The backend SHALL pass `cargo clippy --all-targets -- -D warnings` and
`cargo fmt --all -- --check`, including test and example targets, with zero warnings and
zero errors.

#### Scenario: Clippy over all targets succeeds
- **WHEN** a developer runs `cargo clippy --all-targets -- -D warnings` in `backend/` on a clean checkout
- **THEN** the command exits with status 0 and prints no warnings

#### Scenario: Lint violation in a test target is caught
- **WHEN** a `#[cfg(test)]` module contains a lint violation such as a `match` expression equivalent to `matches!`
- **THEN** `cargo clippy --all-targets -- -D warnings` exits with a non-zero status

### Requirement: Frontend lint gate
The frontend SHALL provide an ESLint configuration and expose a `lint` package script, so
that `pnpm lint` (equivalently `npx eslint .`) lints the TypeScript and React sources and
exits successfully on a clean checkout.

#### Scenario: Lint script exists and passes
- **WHEN** a developer runs `pnpm lint` in `frontend/` on a clean checkout
- **THEN** the command exits with status 0 and reports no lint errors

#### Scenario: Lint script fails on a violation
- **WHEN** a source file contains a lint violation
- **THEN** `pnpm lint` exits with a non-zero status and reports the offending file

### Requirement: Frontend type-check and test gates
The frontend SHALL pass `npx tsc -b` and the full Vitest suite (`pnpm test`) on a clean
checkout.

#### Scenario: Type-check and tests pass
- **WHEN** a developer runs `npx tsc -b` and then `pnpm test` in `frontend/`
- **THEN** both commands exit with status 0 and all tests pass

### Requirement: CI enforces all quality gates
The CI workflow SHALL run the backend gates and the frontend gates on every pull request
targeting `main` or `development`, installing frontend dependencies with pnpm and a frozen
lockfile.

#### Scenario: Pull request runs both stacks
- **WHEN** a pull request targets `main` or `development`
- **THEN** CI runs backend tests, backend format check and clippy with `--all-targets`, plus frontend lint, type-check and tests
- **AND** the workflow fails if any of those commands exits with a non-zero status
