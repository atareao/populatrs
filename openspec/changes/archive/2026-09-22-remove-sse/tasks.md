# Tasks — Remove dead SSE infrastructure

## RED

- [x] Read spec scenarios
- [x] Write failing characterization tests? No — this is a removal, no new behavior to test. Existing tests act as the characterization baseline.

## GREEN

- [x] `src/routes/logs.rs`: Remove `stream()`, `log_layer()`, `LogLayer`, `LogVisitor`, SSE imports
- [x] `src/auth.rs`: Remove `LogEntry` struct, `log_tx` from `AppState`, `broadcast` import
- [x] `src/main.rs`: Remove log_layer creation + `.with(log_layer)`, remove `log_tx` from AppState literal
- [x] `src/routes/mod.rs`: Remove `/api/logs/stream` route
- [x] Verify: `cargo check` ✓
- [x] Verify: `cargo test` ✓ (71/71)
- [x] Verify: `cargo clippy -- -D warnings` ✓
- [x] Verify: `cargo fmt` ✓

## REFACTOR

- [x] No refactoring needed beyond removal — clean separation already achieved

## ARCHIVE

- [x] Mark tasks complete
- [x] Run `openspec archive remove-sse --yes`
- [ ] Post-archive cleanup