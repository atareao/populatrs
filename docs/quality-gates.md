# Quality gates

## Qué es

Puertas de calidad obligatorias del repositorio, ejecutadas localmente y en CI. Alinean el
proyecto con lo prescrito en `AGENTS.md`: backend con formato, clippy estricto y tests;
frontend con lint, type-check y tests.

Origen: issue #78 · change OpenSpec `clippy-all-targets-and-frontend-lint` · plan
`plans/PLAN-002.md`.

## Comandos

### Backend (`backend/`)

| Puerta | Comando |
|---|---|
| Tests | `cargo test` |
| Formato | `cargo fmt --all -- --check` |
| Lint estricto | `cargo clippy --all-targets -- -D warnings` |

`--all-targets` es la clave: incluye los targets de test y ejemplo, donde vivía el único lint
preexistente (`clippy::match_like_matches_macro` en el test `test_state_mismatch` de
`backend/src/routes/auth_routes.rs`), invisible para el `cargo clippy -- -D warnings` que se
usaba antes. El lint se corrigió reescribiendo la expresión con `matches!`, sin `#[allow]`.

### Frontend (`frontend/`)

| Puerta | Comando |
|---|---|
| Lint | `pnpm lint` (equivalente a `npx eslint .`) |
| Type-check | `npx tsc -b` |
| Tests | `pnpm test` (vitest: 13 ficheros / 165 tests) |

## ESLint

- Configuración: `frontend/eslint.config.js`, flat config ESM.
- Presets: `@eslint/js` recommended + `typescript-eslint` recommended (sin parser type-aware)
  + `eslint-plugin-react-hooks` (`recommended-latest`) + `eslint-plugin-react-refresh`
  (`only-export-components` en `warn`).
- Globals: browser + ES2022. Ignorados: `dist`, `node_modules`, `coverage` y los ficheros de
  configuración.
- devDependencies: `eslint` 10.14.0, `@eslint/js` 10.0.1, `typescript-eslint` 8.72.2,
  `eslint-plugin-react-hooks` 7.1.1, `eslint-plugin-react-refresh` 0.5.7, `globals` 17.14.0.
- Toolchain fijada con `"packageManager": "pnpm@12.15.0"` en `frontend/package.json`; el
  lockfile es `pnpm-lock.yaml` (`lockfileVersion: '9.0'`) y CI lo instala con
  `--frozen-lockfile`.
- Estado actual: 0 errores y 1 warning conocido (`CRON_PRESETS` exportado junto al componente
  en `Settings.tsx:42`).

### Excepciones documentadas

Hay 5 `// eslint-disable-next-line react-hooks/set-state-in-effect` puntuales (regla nueva de
react-hooks v7) en `useAuth.ts`, `FeedList.tsx`, `LogsPage.tsx`, `PublisherList.tsx` y
`OAuthCallback.tsx`: son loaders fetch-on-mount que fijan el flag de carga de forma síncrona.
Cada disable lleva su motivo en la misma línea. Resolverlos de verdad exige migrar a una
librería de data-fetching, fuera del alcance de este cambio. No se permite ningún disable
global.

## CI

`.github/workflows/ci.yml`, disparado en `pull_request` a `main` y `development`, con dos
jobs en paralelo:

- `test` (backend): `cargo test`, `cargo fmt --check` y `cargo clippy --all-targets -D warnings`.
- `frontend`: `pnpm/action-setup@v4` (lee `packageManager` de `frontend/package.json`), Node 24
  con caché de pnpm, `pnpm install --frozen-lockfile`, `pnpm lint`, `npx tsc -b` y `pnpm test`.

## Cómo añadir una excepción

1. Prefiere arreglar el código.
2. Si la regla es inaplicable, usa un disable por línea con `-- motivo`.
3. Nunca desactives una regla en bloque ni bajes un preset.

## Decisiones y follow-ups

- El lint de clippy se arregló reescribiendo el `match` con `matches!`, no con `#[allow]`.
- ESLint se adopta sin lint type-aware por coste inicial; si se adopta
  `recommendedTypeChecked`, habrá que añadir `parserOptions.project` y `globals.vitest` para
  los tests.
- Follow-ups: eliminar el estado write-only `retryPolicy`/`publishSettings` de `Settings.tsx`;
  usar inicializador perezoso en `OAuthCallback.tsx` para retirar su disable; mover
  `CRON_PRESETS` a un módulo de constantes.
