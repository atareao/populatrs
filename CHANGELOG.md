# Changelog
## [0.4.9] - 2026-08-18

### Bug Fixes

- Persist refreshed X tokens to database by passing DB reference to PublisherManager
- Persist refreshed X tokens to database

### Miscellaneous Tasks

- Release v0.4.9
## [0.4.8] - 2026-08-13

### Documentation

- Add crate metadata (readme, homepage, keywords, categories)

### Features

- Add retry policy with exponential backoff for publishing (#31)
- Two-step X publishing with reply thread
- Two-step X publishing with reply thread

### Miscellaneous Tasks

- Release v0.4.8
## [0.4.7] - 2026-08-09

### Bug Fixes

- Add structured error logging with request/response context to all publishers

### Miscellaneous Tasks

- Release v0.4.7
## [0.4.6] - 2026-08-07

### Miscellaneous Tasks

- Release v0.4.6
## [0.4.5] - 2026-08-07

### Features

- Add repopulate option for failed publisher results

### Miscellaneous Tasks

- Release v0.4.5
## [0.4.4] - 2026-07-27

### Bug Fixes

- *(ci)* Use GH_PAT instead of GITHUB_TOKEN to trigger release workflow

### Miscellaneous Tasks

- Release v0.4.4
## [0.4.3] - 2026-07-27

### Miscellaneous Tasks

- Release v0.4.3

### Other

- Upgrade react-router-dom to v8.3.0 (CSRF security fix) (#27) (#28)
## [0.4.2] - 2026-07-27

### Miscellaneous Tasks

- Release v0.4.2
## [0.4.0] - 2026-07-27

### Bug Fixes

- *(ci)* Add --manifest-path backend/ to all cargo commands
- *(ci)* Prevent recursive Prepare Release + explicit Release trigger

### Other

- 0.4.0 — Cron presets, YouTube publisher, docs
## [0.3.10] - 2026-07-27

### Bug Fixes

- *(ci)* Proper merge sync of development with main after release
- *(ci)* Proper merge sync of development with main after release

### Miscellaneous Tasks

- Release v0.3.10
## [0.3.9] - 2026-07-27

### Bug Fixes

- *(ci)* Use github.token instead of secrets.GH_PAT for sync step
- *(ci)* Proper merge sync of development with main after release
- *(ci)* Use github.token instead of secrets.GH_PAT for sync step

### Miscellaneous Tasks

- Release v0.3.9
## [0.3.8] - 2026-07-27

### Bug Fixes

- *(threads)* Persist token_expires_at to avoid 452 error on restart
- *(threads)* Persist token_expires_at to avoid 452 error on restart

### Miscellaneous Tasks

- Release v0.3.8
## [0.3.7] - 2026-07-26

### Miscellaneous Tasks

- Release v0.3.7

### Other

- 0.4.0 — OIDC state, timezone-aware cron, Threads fix
## [0.3.6] - 2026-07-26

### Bug Fixes

- *(threads)* Fix access_token as query param and user_id numeric parsing

### Features

- OIDC state, timezone-aware cron, Threads fix, cron presets

### Miscellaneous Tasks

- Release v0.3.6
## [0.3.5] - 2026-07-25

### Documentation

- Rewrite README to reflect current web app architecture
- Add populatrs.env.example with all env vars and fix README table

### Miscellaneous Tasks

- Release v0.3.5

### Other

- V0.4.0

### Styling

- Add app icon to favicon and login page
## [0.3.4] - 2026-07-23

### Features

- *(oauth)* Add OAuth flows for Threads and Mastodon publishers
- Improve publisher UI - replace switches with emojis, hide feed ID column, add test result modals
- Improve publisher UI and add OAuth flows for Threads/Mastodon
- Add YouTube API config, publisher manager, and feed publish endpoint
- Add cron-based schedule with settings UI and dashboard timing (#19)

### Miscellaneous Tasks

- Use GH_PAT for git push in release-prepare to trigger Release workflow
- Fix release-prepare to use GH_PAT for git push
- Release v0.3.4
## [0.3.3] - 2026-07-23

### Bug Fixes

- LinkedIn OAuth redirect_uri must match backend URL exactly
- *(linkedin)* Normalize newlines and fix user_id in OAuth callback
- *(linkedin)* Normalize newlines and fix user_id in OAuth callback

### Miscellaneous Tasks

- Release v0.3.3
- Release v0.3.3
## [0.3.2] - 2026-07-23

### Features

- *(publishers)* Test, delete, edit fix and enabled toggle for publishers (#7)
- Add feed publication history with configurable retention (#9)
- Per-feed template overrides and publisher template refactor

### Miscellaneous Tasks

- Release v0.3.2

### Other

- 0.4.0
## [0.3.1] - 2026-07-21

### Miscellaneous Tasks

- Release v0.3.1
## [0.3.0] - 2026-07-21

### Features

- *(publishers)* Test, delete, edit fix and enabled toggle for publishers (#7) (#8)

### Miscellaneous Tasks

- Release v0.3.0
## [0.2.0] - 2026-07-15

### Features

- Storage config endpoint, schedule separation, pnpm migration, and dashboard improvements

### Miscellaneous Tasks

- Release v0.2.0

### Refactor

- Reestructuración completa a modelo Alloy
## [0.1.5] - 2026-06-23

### Miscellaneous Tasks

- Release v0.1.5
## [0.1.4] - 2026-06-23

### Bug Fixes

- Fix clippy warnings across codebase
- Fix clippy warnings across codebase

### Miscellaneous Tasks

- Release v0.1.4
- Add crates.io publish job to release workflow
- Add crates.io publish job to release workflow
## [0.1.3] - 2026-06-23

### Documentation

- Update README with professional badges and fixed URLs
- Update README with professional badges and fixed URLs

### Miscellaneous Tasks

- Configure gitflow with CI/CD workflows
- Use GITHUB_TOKEN instead of GH_PAT, add docker publish to ghcr
- Release v0.1.3

### Other

- 22:40.577175Z[0m [31mERROR[0m [2mdime[0m[2m:[0m You exceeded your current quota, please check your plan and billing details. For more information on this error, read the docs: https://platform.openai.com/docs/guides/error-codes/api-errors.
- V0.1.3
