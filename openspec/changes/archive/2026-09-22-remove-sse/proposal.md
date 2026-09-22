# Remove dead SSE infrastructure

## Why

The SSE log streaming (`/api/logs/stream`, `log_tx` broadcast, `LogEntry`, `LogLayer`)
is dead code. The frontend `LogsPage` has never consumed the SSE stream — it uses REST
polling with `fetchFeedLogs()` and a manual Refresh button. This is ~145 lines of
infrastructure with zero consumers.

## What Changes

Four files in `backend/src/` are modified:

| File | Change |
|---|---|
| `routes/logs.rs` | Remove `stream()`, `log_layer()`, `LogLayer`, `LogVisitor`, SSE imports |
| `auth.rs` | Remove `LogEntry` struct, `log_tx` from `AppState`, `broadcast` import |
| `main.rs` | Remove log_layer creation + `.with(log_layer)` calls, remove `log_tx` from AppState literal |
| `routes/mod.rs` | Remove `/api/logs/stream` route |

No functional change: the `/api/logs/history`, retention, and republish endpoints remain
untouched. No frontend changes needed.