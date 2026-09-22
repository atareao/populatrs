# sse Specification

## Purpose
Document the removal of the SSE log streaming infrastructure from the backend.
The endpoint, broadcast channel, LogEntry type, and tracing layer were dead code
— the frontend LogsPage uses REST polling, never SSE.

## Requirements

### Requirement: LogsPage uses REST polling exclusively

The LogsPage SHALL use `GET /api/logs/history` with pagination parameters for
loading feed publication logs. No SSE endpoint exists.

#### Scenario: History loads via REST pagination

**GIVEN** a user visits the LogsPage  
**WHEN** the page loads  
**THEN** `GET /api/logs/history?limit=100&offset=0` is called  
**AND** the response contains paginated entries with publisher results  
**AND** no EventSource/SSE connection is established

### Requirement: No SSE log streaming endpoint

The path `GET /api/logs/stream` SHALL NOT exist. Any request to this path SHALL
receive a 404 Not Found response.

#### Scenario: SSE endpoint returns 404

**GIVEN** the SSE infrastructure is removed  
**WHEN** a client sends `GET /api/logs/stream`  
**THEN** the server responds with HTTP 404 Not Found

### Requirement: No LogEntry type or broadcast channel

The type `LogEntry` and the `broadcast::Sender<LogEntry>` field in `AppState` SHALL
NOT exist. The `tokio::sync::broadcast` module SHALL NOT be imported in `auth.rs`.

#### Scenario: AppState has no log_tx field

**GIVEN** the SSE removal changes are applied  
**WHEN** inspecting `AppState`  
**THEN** there is no `log_tx` field  
**AND** `LogEntry` is not defined in `auth.rs`

### Requirement: Tracing works without LogLayer

The tracing subscriber SHALL function correctly using only `EnvFilter` and the fmt
layer, without any SSE broadcast layer.

#### Scenario: Tracing without LogLayer

**GIVEN** the tracing subscriber no longer includes `.with(log_layer)`  
**WHEN** the application starts and logs a message  
**THEN** the message appears in stdout in the configured format (pretty or json)
