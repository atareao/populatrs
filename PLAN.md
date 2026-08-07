# Retry Policy for Publishing with Exponential Backoff — Implementation Plan

## Objective

Add a configurable global retry policy with exponential backoff for failed publish/republish attempts, stored as JSON in the `settings` table and configurable from the Settings UI.

## Architecture

- **Inline retry**: synchronous `tokio::time::sleep` within the same task — no background queue.
- **Replace on retry**: each retry attempt calls `replace_publish_result()` so the UI tag reflects the latest attempt.
- **Global only**: the `RetryPolicy` struct is stored in the `settings` table under key `retry_policy`; per-feed `max_retries`/`retry_delay_seconds` fields are ignored.
- **Helper function**: `publish_with_retry()` in `lib.rs` encapsulates the retry loop, accepting a publisher reference, post, optional template, retry policy, and db reference.

## Files Changed

| File | Action |
|---|---|
| `backend/src/models.rs` | Add `RetryPolicy` struct with `Default` impl |
| `backend/src/db.rs` | Add `get_retry_policy()` / `set_retry_policy()` methods |
| `backend/src/lib.rs` | Add `publish_with_retry()` helper + `calculate_backoff()`; modify `run_feed_check()` to use retry per-publisher |
| `backend/src/routes/retry.rs` | **NEW** — GET/PUT `/api/settings/retry-policy` |
| `backend/src/routes/mod.rs` | Register `retry` module and routes |
| `backend/src/routes/logs.rs` | Modify `republish()` to use `publish_with_retry()` |
| `frontend/src/api/http.ts` | Add `RetryPolicy` interface, `fetchRetryPolicy()`, `updateRetryPolicy()` |
| `frontend/src/pages/Settings.tsx` | Add Retry Policy configuration card |

## API Endpoints

### GET /api/settings/retry-policy

Returns the current retry policy (or the default if not yet saved).

### PUT /api/settings/retry-policy

Accepts the same shape, validates fields, persists to `settings` table.

## Data Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,            // default: 3
    pub base_delay_seconds: u64,     // default: 5
    pub max_delay_seconds: u64,      // default: 300 (5 min)
    pub backoff_multiplier: f64,     // default: 2.0
}
```

## Implementation Steps

### Step 1: `backend/src/models.rs` — Add `RetryPolicy` struct
### Step 2: `backend/src/db.rs` — Add retry policy DB methods
### Step 3: `backend/src/lib.rs` — Add `publish_with_retry()` + `calculate_backoff()`; modify `run_feed_check()`
### Step 4: `backend/src/routes/retry.rs` — NEW file with GET/PUT
### Step 5: `backend/src/routes/mod.rs` — Register retry routes
### Step 6: `backend/src/routes/logs.rs` — Modify `republish()` to use retry
### Step 7: `frontend/src/api/http.ts` — Add RetryPolicy API
### Step 8: `frontend/src/pages/Settings.tsx` — Add Retry Policy card

## Testing Notes

1. Unit test `RetryPolicy::default()` — verify all four fields match spec.
2. Unit test `calculate_backoff()` — verify attempt 1 = base, attempt N capped at max.
3. Integration test `get_retry_policy` — returns default when no setting stored.
4. Integration test `set_retry_policy` → `get_retry_policy` — roundtrip.
5. Manual test: configure retry policy via Settings UI, trigger a publish to a misconfigured publisher, observe retries in logs.