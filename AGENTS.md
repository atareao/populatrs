# AGENT DIRECTIVES — populatrs (OpenSpec SDD + TDD)

## I. Reglas innegociables (siempre / nunca)

- **Phase 0 — Legacy:** si tocas código sin specs/tests previos, crea primero el spec base (`openspec/specs/<module>/spec.md`) y tests de caracterización en GREEN.
- **Phase 1 — SDD:** prohibido escribir código o tests nuevos antes de que exista un change proposal en `openspec/changes/<feature>/` y el usuario lo apruebe explícitamente.
- **Phase 2 — TDD:** con el spec aprobado, desarrollar estrictamente test-first por CLI (RED → GREEN → REFACTOR).
- **Strict Verification:** ejecuta los tests siempre por terminal; nunca asumas pass/fail sin salida CLI.
- **🚫 NO-SKIP:** ninguna orden del usuario ("adelante", "ejecuta", "go ahead", "sounds good", "looks good", etc.) salta SDD → TDD. Ante orden ambigua, responde primero: *"¿Quieres que genere el change proposal en openspec/changes/ primero?"*.
- **🔁 FOLLOW-UP:** cada ronda de ajustes sobre un cambio ya archivado necesita SU PROPIO change proposal (p. ej. `ui-fixes-v2`); un cambio archivado no autoriza tocar código.
- **📁 SPEC FILE NAMING:** el delta spec DEBE llamarse `spec.md`: `specs/ui/layout/spec.md` (no `specs/ui/layout.md`); otro nombre lo ignora `openspec archive`.
- **✅ APPROVED MARKER:** tras aprobación explícita, `touch openspec/changes/<feature>/.approved` (evita el warning de `require-proposal.sh`).
- **🧹 POST-ARCHIVE CLEANUP:** tras `openspec archive <feature> --yes`, verifica que `openspec/changes/<feature>/` se eliminó; si no, `rm -rf openspec/changes/<feature>/`.
- **⚠️ PRE-FLIGHT OBLIGATORIO:** antes de CUALQUIER tool call que lea o escriba `backend/src/*.rs` o `frontend/src/*.{tsx,ts}`, ejecuta `source openspec/require-proposal.sh`; si falla, párate y crea el proposal primero.
- **⚠️ ARCHIVE PRE-FLIGHT:** antes de archivar, comprueba que la ruta destino en `openspec/specs/` no tenga ya un spec en conflicto (p. ej. `specs/ui/spec.md` vs `specs/ui/layout/spec.md`); renombra/mueve primero.

## II. Comandos (CLI)

- Backend (Rust): `cargo test` · `cargo clippy -- -D warnings` · `cargo fmt -- --check`
- Frontend (React+TS): `npx vitest run` (o `pnpm test`) · `npx tsc -b` · `npm run lint` / `npx eslint .`
- Dev backend (desde `backend/`): `HOST=0.0.0.0 PORT=3044 RUST_LOG=info cargo run` (sin OIDC = bypass automático de auth)
- Dev frontend (desde `frontend/`): `pnpm install` · `pnpm dev` (5173 → proxy a backend :3044) · `pnpm build` (→ `dist/`)
- OpenSpec: `openspec new change <feature-name>` · `openspec archive <feature-name> --yes`
- Just: `just list` · `just build` · `just push` · `just version`

## III. Restricciones vigentes

- **Frontend estático:** nada de `rust-embed`; el servidor sirve `./dist/` con `tokio::fs::read`.
- **pnpm workspaces**, no npm, para el frontend.
- **Auth:** OIDC obligatorio en producción (PocketID); en desarrollo hay bypass automático (`/auth/dev-login?email=...`).
- **Despliegue:** solo Quadlet. No usar docker-compose ni podman-compose.
- **Tests:** Rust unitarios en el mismo fichero bajo `#[cfg(test)]`, integración/API en `tests/`; React componentes en `src/components/`, hooks en `src/hooks/`, tests colocados `*.test.tsx` con `@testing-library/react` + `user-event`.

## IV. Status tags (prefijar cada update)

```text
[LEGACY - INSPECT] Creating baseline spec & characterization tests.
[OPENSPEC - DRAFT] Generating change proposal in openspec/changes/...
[OPENSPEC - WAITING] Spec generated. Awaiting user review and approval.
[TDD - RED] Creating tests for scenario <Name> -> Running CLI tests.
[TDD - GREEN] Implementing minimal code -> Running CLI tests & type checks.
[TDD - REFACTOR] Refactoring code -> Running Clippy/ESLint & tests.
[OPENSPEC - ARCHIVE] Archiving change into openspec/specs/.
```

## Dónde está el resto

| Tema | Fichero |
|---|---|
| Workflow SDD/TDD completo (cláusulas literales, fases 0-2, stack y formato de respuesta) | `docs/workflow-sdd-tdd.md` |
| Estado del proyecto (estructura, desarrollo, env vars, arquitectura, DB, scheduler, pipeline, despliegue) | `docs/arquitectura-populatrs.md` |
| Refresco de tokens OIDC | `docs/oidc-token-refresh.md` |
