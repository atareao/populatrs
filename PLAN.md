# PLAN: Replace Check Interval with Cron Expression

## Goal
Replace the fixed `CHECK_INTERVAL` (minutes) scheduler with a cron expression (`"0 */6 * * *"`). The scheduler parses the cron expression and sleeps until the next tick instead of a fixed interval.

## Changes

### Backend

| File | Change |
|---|---|
| `Cargo.toml` | Add `cron = "0.15"` crate |
| `config.rs` | Remove `default_interval_minutes: u64`, add `default_cron_expression: String` from `CHECK_INTERVAL` → `SCHEDULE_CRON` (default: `"0 * * * *"`) |
| `models.rs` | `ScheduleConfig.default_interval_minutes: u64` → `ScheduleConfig.cron_expression: String`. Keep `timezone`. |
| `db.rs` | `get_schedule`/`set_schedule`: use `schedule_cron` setting key instead of `schedule_interval`. Migration: convert old `schedule_interval` to cron if upgrading. `get_stats` inline query same change. |
| `main.rs` | `feed_scheduler_loop`: parse cron, calc next DateTime with `cron::Schedule`, `tokio::time::sleep_until` instead of fixed sleep. Remove `interval` param to `run_feed_check`. |
| `lib.rs` | Remove `default_interval_minutes` param from `run_feed_check` (already unused in `check_all_feeds`). |
| `routes/schedule.rs` | Update error fallback to `"cron_expression": "0 * * * *"` |
| `routes/status.rs` | Return `"cron_expression"` and `"timezone"` instead of `"interval_minutes"` |

### Frontend

| File | Change |
|---|---|
| `api/http.ts` | `ScheduleConfig`: `default_interval_minutes` → `cron_expression`. `DashboardStatus.schedule.interval_minutes` → `cron_expression` |
| `pages/Schedule.tsx` | Replace `InputNumber` for minutes with `Input` for cron expression. Update text from "minutes" to cron |
| `pages/Dashboard.tsx` | Show cron expression instead of `interval_minutes` min |
| `pages/Settings.tsx` | Same form change as Schedule |
| Tests | Update all mock data to use `cron_expression` |

## Migration
On startup, if `schedule_cron` setting doesn't exist in DB:
- If `schedule_interval` exists, convert: `*/N * * * *` where N = old interval
- Otherwise use default `"0 * * * *"`
Then write the new `schedule_cron` setting.

## Removed
- `CHECK_INTERVAL` env var (replaced by `SCHEDULE_CRON` with default `"0 * * * *"`)
- `default_interval_minutes` field everywhere
- `schedule_interval` DB key
- Unused `_default_interval_minutes` param in `check_all_feeds`