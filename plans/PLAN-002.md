# PLAN-002 — Quality gates: clippy --all-targets y lint de frontend

- **Issue:** #78 — fix(ci): clippy --all-targets falla y el script lint del frontend no existe
- **OpenSpec change:** `clippy-all-targets-and-frontend-lint`
- **Estado:** propuesta (pendiente de aprobación)
- **Fecha:** 2026-11-11

## Objetivo
Alinear las puertas de calidad del repositorio con lo prescrito en `AGENTS.md`: que
`cargo clippy --all-targets -- -D warnings` pase, que el frontend tenga un script `lint`
respaldado por ESLint, y que CI haga cumplir ambos stacks en cada pull request.

## Baseline medido
- `cd backend && cargo clippy --all-targets -- -D warnings` → falla con 1 error
  `match_like_matches_macro` en `backend/src/routes/auth_routes.rs:867` (módulo `#[cfg(test)]`).
- `cd frontend && pnpm lint` → no existe el script.
- `cd frontend && npx vitest run` → 13 ficheros / 165 tests OK.
- `cd frontend && npx tsc -b` → OK.
- `.github/workflows/ci.yml` → sin `--all-targets` en clippy y sin job de frontend.

## Tareas
- [x] 1. Backend: reproducir el fallo de clippy (`--all-targets`), reescribir el `match` del
test de `auth_routes.rs` con `matches!`, y verificar clippy + test + fmt en verde.
- [x] 2. Frontend: añadir ESLint (flat config) y el script `lint` a `package.json`,
instar dependencias y dejar `pnpm lint` en verde sin tocar la semántica de `src/`.
- [x] 3. CI: añadir `--all-targets` al job de clippy y crear un job `frontend`
  (pnpm frozen-lockfile + lint + `tsc -b` + vitest) en `.github/workflows/ci.yml`.
- [ ] 4. Documentar en `docs/quality-gates.md`, archivar este plan y el change de OpenSpec.

## Criterios de aceptación
- [x] `cargo clippy --all-targets -- -D warnings` sale con 0.
- [x] `pnpm lint` en `frontend/` sale con 0.
- [x] CI pasa los jobs de backend y frontend en un pull request.
- [x] Sin cambios de comportamiento en producción (el arreglo de Rust es solo de test).

## Fuera de alcance
- Prettier o formateo JS/TS en CI.
- La automatización de release rota (`release-prepare.yml`, token `GH_TOKEN` 401 y
  `git-cliff` sin sección de versión), que se mantiene como tarea aparte del `HANDOFF.md`.