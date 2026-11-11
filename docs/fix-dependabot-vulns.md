# Fix de vulnerabilidades Dependabot (31 alertas)

- **Issue:** #75
- **PR:** #76 (feature/fix-dependabot-vulns → development), PR #77 (development → main)
- **Release:** v0.4.23
- **Fecha:** 2026-11-11

## Qué es

Actualización de dependencias para resolver las 31 alertas Dependabot abiertas. No hubo cambios en código fuente: solo lockfiles y bump de rangos en `frontend/package.json`.

## Ficheros implicados

- `backend/Cargo.lock`
- `frontend/package.json`
- `frontend/pnpm-lock.yaml`
- `plans/PLAN-001.md` (archivado)

## Cómo funciona

1. `cargo update -p rustls` en `backend/` resolvió la cadena rustls.
2. `pnpm update undici vite source-map-js postcss browserslist baseline-browser-mapping vitest @vitest/mocker` en `frontend/` refrescó los lockfiles dentro de los rangos semver ya declarados.
3. Verificación: `cargo test`, `cargo fmt -- --check`, `tsc -b`, `vitest run`.
4. PR #76 → `development`; luego PR #77 `development` → `main` (dispara el release).

## Versiones

| Paquete | Antes | Después | Alertas |
|---|---|---|---|
| backend rustls | 0.23.41 | 0.23.46 | #31-34 |
| backend rustls-webpki | 0.103.13 | 0.103.16 | (cadena rustls) |
| frontend undici | 7.28.0 | 7.30.2 | #3-7,17,19-21,24,25,29,42-45 |
| frontend vite | 6.4.3 | 6.4.6 | #35-39 |
| frontend source-map-js | 1.2.1 | 1.2.2 | #26 |
| frontend postcss | 8.5.19 | 8.5.32 | #8 |
| frontend browserslist | 4.28.6 | 4.29.5 | #11 |
| frontend baseline-browser-mapping | 2.10.43 | 2.11.35 | #15 |
| frontend vitest / @vitest/mocker | 4.1.11 | 4.1.12 | #27,28 |

## Cómo verificar

```bash
cd backend && cargo test && cargo fmt -- --check
cd frontend && npx tsc -b && npx vitest run
gh api --paginate "repos/atareao/populatrs/dependabot/alerts?state=open" --jq 'length'   # → 0
```

## Decisiones

- Los 4 PRs de Dependabot que apuntaban directamente a `main` (#67, #68, #70, #73) se cerraron como superseded, porque violaban Git Flow (todo entra por PR desde una rama `feature/*` y pasa por `development`).
- Se trató como chore de mantenimiento, sin change proposal de OpenSpec (no hay código ni tests nuevos).

## Follow-ups / problemas detectados (fuera de alcance)

1. **`release-prepare.yml` falla en el paso final** `gh workflow run Release`: `HTTP 401: Bad credentials`. El secret `GH_TOKEN` está caducado. Consecuencia: el tag `v0.4.23` y el commit de release sí se crearon, pero **no se dispara el workflow `Release`** (build/publicación).
2. **`git-cliff` no genera la entrada de la nueva versión** en `CHANGELOG.md` (el commit de release v0.4.23 no añadió sección `## [0.4.23]`).
3. **El paso `Sync development with main` se salta** al fallar el job → `development` queda por detrás de `main` tras cada release (hay que sincronizar con un PR `chore/sync-development-with-main`).
4. Issue #78: `cargo clippy --all-targets` falla (lint preexistente en `backend/src/routes/auth_routes.rs:867`) y no existe script `lint` en `frontend/package.json`.
