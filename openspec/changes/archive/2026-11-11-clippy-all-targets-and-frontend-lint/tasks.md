# Tasks

## 1. Backend: fix clippy over all targets

- [x] 1.1 Reproduce the failure: `cd backend && cargo clippy --all-targets -- -D warnings` exits non-zero with `match_like_matches_macro` at `backend/src/routes/auth_routes.rs:867` (RED baseline).
- [x] 1.2 Rewrite the test expression in `backend/src/routes/auth_routes.rs` using `matches!` exactly as proposed by clippy: `let is_mismatch = !matches!((&cb_state, &stored), (Some(cb), Some((ref stored_state, _, _))) if stored_state == cb);`
- [x] 1.3 Verify GREEN: `cargo clippy --all-targets -- -D warnings`, `cargo test` and `cargo fmt --all -- --check` all exit 0.
- [x] 1.4 Audit with `@rust-reviewer` before moving on.

## 2. Frontend: lint gate

- [x] 2.1 Record the RED baseline: `cd frontend && pnpm lint` fails because the script does not exist.
- [x] 2.2 Add ESLint devDependencies (`eslint`, `@eslint/js`, `typescript-eslint`, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`, `globals`) and the "lint": "eslint ." script to `frontend/package.json`.
- [x] 2.3 Add `frontend/eslint.config.js` with the flat config described in `design.md` (TS + React sources, `dist/` ignored).
- [x] 2.4 Run `pnpm install` and verify GREEN: `pnpm lint` exits 0 with no errors.
- [x] 2.5 Fix any violations reported in `frontend/src/` (targeted fixes or justified per-line disables only).
- [x] 2.6 Re-verify the other gates are unaffected: `npx tsc -b` and `pnpm test` (165 tests) still pass.

## 3. CI enforcement

- [x] 3.1 Add `--all-targets` to the clippy step in `.github/workflows/ci.yml`.
- [x] 3.2 Add a `frontend` job: checkout, setup pnpm (version pinned as in `design.md`), setup Node, `pnpm install --frozen-lockfile`, `pnpm lint`, `npx tsc -b`, `pnpm test`, with the same `pull_request` triggers (`main`, `development`).
- [x] 3.3 Validate the workflow file syntax locally (e.g. `actionlint` if available) and confirm both jobs are listed.

## 4. Documentation and closure

- [x] 4.1 Write `docs/quality-gates.md` describing the gates, the commands, the ESLint setup, the CI jobs and how to run them locally.
- [ ] 4.2 Tick every checkbox in `plans/PLAN-002.md` and archive it as `plans/PLAN-002-ARCHIVED.md`.
- [ ] 4.3 Archive the OpenSpec change (`openspec archive clippy-all-targets-and-frontend-lint --yes`) and confirm the delta landed in `openspec/specs/ci/quality-gates/spec.md`.
- [ ] 4.4 Commit with gitmoji conventional commits on a `feature/` branch, open a PR to `development`, merge, then release to `main`.

## 5. Acceptance criteria

- [x] 5.1 `cargo clippy --all-targets -- -D warnings` exits 0.
- [x] 5.2 `pnpm lint` exits 0 in `frontend/`.
- [x] 5.3 CI runs and passes both the backend and frontend jobs on a pull request.
