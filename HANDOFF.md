# HANDOFF

## Estado (2026-11-11)

- **31 vulnerabilidades Dependabot RESUELTAS** (`gh api .../dependabot/alerts?state=open` → 0).
- Mergeado en `main` como release **v0.4.23**.
- Issue #75 · PR #76 (→development) · PR #77 (→main) · Plan `plans/PLAN-001-ARCHIVED.md` · Doc `docs/fix-dependabot-vulns.md`.

## Tarea pendiente (siguiente sesión)

1. **#78** — arreglar `cargo clippy --all-targets -- -D warnings` (lint `match_like_matches_macro` en `backend/src/routes/auth_routes.rs:867`) y añadir script `lint` (eslint) a `frontend/package.json`. Requiere change proposal OpenSpec.
2. **Automatización de release rota** (preexistente):
   - `release-prepare.yml` → paso `Trigger Release workflow`: `GH_TOKEN` inválido (HTTP 401). Hay que regenerar el secret.
   - `git-cliff` no añade la sección de la nueva versión al `CHANGELOG.md`.
   - El paso `Sync development with main` se salta al fallar el job → sincronizar `development` desde `main` con un PR.
3. Verificar que el workflow `Release` publique v0.4.23 una vez arreglado el token.

## Comandos útiles

```bash
gh api --paginate "repos/atareao/populatrs/dependabot/alerts?state=open" --jq 'length'
gh run list --branch main --limit 5
```
