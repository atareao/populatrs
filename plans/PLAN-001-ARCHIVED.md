# PLAN-001 — Resolver 31 vulnerabilidades de dependencias (Dependabot)

- **Issue:** https://github.com/atareao/populatrs/issues/75
- **Estado:** completado (archivado)
- **Tipo:** chore (mantenimiento de dependencias)
- **Rama:** `feature/fix-dependabot-vulns` (desde `development`) → PR a `development`
- **SDD:** exento (no hay código ni tests nuevos; solo lockfiles)

## Contexto

31 alertas Dependabot abiertas, todas de dependencias. No hay alertas de secret scanning ni de code scanning.

Versiones actuales en lockfiles vs. primera versión parcheada:

| Ecosistema | Paquete | Actual | Fix | Alertas |
|---|---|---|---|---|
| backend | rustls | 0.23.41 | 0.23.46 | #31, #32, #33, #34 |
| frontend | undici | 7.28.0 | 7.30.1 | #3-7, #17, #19-21, #24, #25, #29, #42-45 |
| frontend | vite | 6.4.3 | 6.4.5 | #35-39 |
| frontend | source-map-js | 1.2.1 | 1.2.2 | #26 |
| frontend | postcss | 8.5.19 | 8.5.23 | #8 |
| frontend | browserslist | 4.28.6 | 4.28.7 | #11 |
| frontend | baseline-browser-mapping | 2.10.43 | 2.11.0 | #15 |
| frontend | @vitest/mocker | — | 4.1.12 | #27, #28 |

Todas las restricciones de `package.json` (`vite ^6.2.2`, `vitest ^4.1.11`) y `Cargo.toml` permiten las versiones parcheadas: basta refrescar lockfiles.

## Tareas

- [x] T1. Sincronizar `development` y crear rama `feature/fix-dependabot-vulns`.
- [x] T2. Backend: `cargo update -p rustls` hasta 0.23.46 (y crates asociados si aplica).
- [x] T3. Frontend: refrescar lockfile para undici, vite, source-map-js, postcss, browserslist, baseline-browser-mapping y vitest/@vitest/mocker.
- [x] T4. Verificar backend: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`.
- [x] T5. Verificar frontend: `pnpm test`, `npx tsc -b`, lint.
- [x] T6. Revisar `git diff --stat` de los lockfiles y confirmar bumps.
- [x] T7. Commit gitmoji (`🔧 chore(deps): ...`), push y PR a `development`.
- [x] T8. Cerrar los PRs de Dependabot a `main` (#67, #68, #70, #73) y confirmar que las alertas se cierran.
- [x] T9. Crear `docs/fix-dependabot-vulns.md` y archivar este plan a `plans/PLAN-001-ARCHIVED.md`.

## Criterio de aceptación

- `cargo test` + `cargo clippy -- -D warnings` en verde.
- `pnpm test` + `tsc` + lint en verde.
- 31 alertas Dependabot cerradas tras el merge.