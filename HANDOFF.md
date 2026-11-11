# HANDOFF

## Estado (2026-11-11)

- **Issue #78 CERRADO** — puertas de calidad alineadas con `AGENTS.md`:
  - `cargo clippy --all-targets -- -D warnings` en verde (el `match` del test `test_state_mismatch` se reescribió con `matches!`, sin `#[allow]`).
  - Frontend con ESLint 10 flat config y script `pnpm lint` → 0 errores, 1 warning conocido (`react-refresh/only-export-components` en `Settings.tsx:42`).
  - CI: `--all-targets` en el job `test` + nuevo job `frontend` (`pnpm install --frozen-lockfile` → `pnpm lint` → `npx tsc -b` → `pnpm test`).
- PRs #81 (→ `development`) y #82 (→ `main`), ambos con los dos jobs de CI en verde. Todo mergeado en `main`.
- OpenSpec: change `clippy-all-targets-and-frontend-lint` archivado → `openspec/specs/ci/quality-gates/spec.md` (4 requisitos). Specs validados con `openspec validate --specs --strict` (4 passed).
- Plan archivado: `plans/PLAN-002-ARCHIVED.md`. Doc nueva: `docs/quality-gates.md`.

## Tarea pendiente (siguiente sesión)

1. **Automatización de release rota** (preexistente, NO resuelta; bloquea publicar lo ya mergeado en `main`):
   - `release-prepare.yml` → paso `Trigger Release workflow`: `GH_TOKEN` inválido (HTTP 401). Regenerar el secret.
   - `git-cliff` no añade la sección de la nueva versión al `CHANGELOG.md`.
   - Tras arreglarlo, publicar la release: el trabajo actual está en `main` como `v0.4.23+13` commits, sin tag nuevo.
2. **Follow-ups de lint del frontend** (del PR #81, no bloqueantes; recuerda la FOLLOW-UP CLAUSE: necesitan su propio change proposal):
   - Eliminar el estado write-only `retryPolicy`/`publishSettings` de `Settings.tsx` y sus 4 setters.
   - `OAuthCallback.tsx`: inicializador perezoso en `useState` para retirar su `eslint-disable`.
   - Mover `CRON_PRESETS` (`Settings.tsx:42`) a un módulo de constantes y subir `react-refresh/only-export-components` a `error` en `frontend/eslint.config.js`.
3. **Inconsistencia detectada**: `AGENTS.md` exige `source openspec/require-proposal.sh` y `source openspec/validate-workflow.sh` como pre-flight, pero ninguno de los dos scripts existe en el repo. Decidir: crearlos o quitar la exigencia de `AGENTS.md`.

## Comandos útiles

```bash
gh api --paginate "repos/atareao/populatrs/dependabot/alerts?state=open" --jq 'length'
gh run list --branch main --limit 5
cd backend && cargo clippy --all-targets -- -D warnings && cargo test && cargo fmt --all -- --check
cd frontend && pnpm lint && npx tsc -b && pnpm test
```
