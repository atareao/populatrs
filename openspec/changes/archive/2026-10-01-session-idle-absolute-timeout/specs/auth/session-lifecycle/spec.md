# Spec Delta

## MODIFIED Requirements

### Requirement: Session validity is determined client-side

The client SHALL expose a session-validity check based on the JWT `exp` claim and the
locally-tracked session window (start time and last activity), allowing a small clock-skew
tolerance. A token that is expired or cannot be decoded, or a session whose idle window or
absolute lifetime has passed, SHALL be reported as invalid. The login decision SHALL use
this check.

#### Scenario: Expired token is reported invalid

- **GIVEN** a stored JWT whose `exp` is earlier than the current time minus the skew allowance
- **WHEN** the client checks session validity
- **THEN** the check reports the session as invalid

#### Scenario: Valid token is reported valid

- **GIVEN** a stored JWT whose `exp` is later than the current time plus the skew allowance
- **AND** the client-tracked session window has not passed
- **WHEN** the client checks session validity
- **THEN** the check reports the session as valid

#### Scenario: Malformed token is reported invalid

- **GIVEN** a stored value that is not a well-formed JWT
- **WHEN** the client checks session validity
- **THEN** the check reports the session as invalid

#### Scenario: Session past the idle window is reported invalid

- **GIVEN** a stored JWT that has not expired
- **AND** the client-tracked last-activity time is older than the idle timeout
- **WHEN** the client checks session validity
- **THEN** the check reports the session as invalid

#### Scenario: Session past the absolute lifetime is reported invalid

- **GIVEN** a stored JWT that has not expired
- **AND** the client-tracked session start is older than the absolute lifetime
- **WHEN** the client checks session validity
- **THEN** the check reports the session as invalid

### Requirement: Silent refresh works with an expired access token

The client SHALL keep the raw stored access token retrievable independently of the validity
check, so the refresh flow can present an expired token to the server, which validates its
signature while ignoring expiration. The server SHALL additionally reject a refresh when the
session has exceeded its idle timeout or its absolute lifetime, and a successful refresh
SHALL NOT extend the absolute lifetime.

#### Scenario: Refresh uses an expired access token

- **GIVEN** an expired access token is stored and a valid refresh token exists server-side
- **AND** the session is within both the idle timeout and the absolute lifetime
- **WHEN** an API call returns HTTP 401 and the client attempts a silent refresh
- **THEN** the client sends the expired access token to `POST /auth/refresh`
- **AND** the server returns a new access token

#### Scenario: Refresh rejected after the idle timeout

- **GIVEN** a stored session whose last recorded activity is older than the idle timeout
- **WHEN** the client calls `POST /auth/refresh`
- **THEN** the server responds with HTTP 401 Unauthorized
- **AND** the session is revoked from storage

#### Scenario: Refresh rejected after the absolute lifetime even when active

- **GIVEN** a stored session whose age exceeds the absolute lifetime
- **AND** the last recorded activity is recent
- **WHEN** the client calls `POST /auth/refresh`
- **THEN** the server responds with HTTP 401 Unauthorized
- **AND** the session is revoked from storage

#### Scenario: Refresh does not extend the absolute lifetime

- **GIVEN** a session within both the idle timeout and the absolute lifetime
- **WHEN** a refresh succeeds
- **THEN** a new access token is returned
- **AND** the session's absolute-lifetime anchor is unchanged

## ADDED Requirements

### Requirement: Session has an absolute lifetime

The server SHALL assign each session an absolute lifetime beginning at login. The anchor
SHALL NOT be extended by refresh or by activity. When a session's age exceeds the absolute
lifetime, the server SHALL reject authenticated requests, revoke the session, and respond
with HTTP 401 Unauthorized.

#### Scenario: Active session reaches the absolute lifetime

- **GIVEN** a session whose age exceeds the absolute lifetime
- **AND** the last recorded activity is recent
- **WHEN** any authenticated request is made
- **THEN** the server responds with HTTP 401 Unauthorized
- **AND** the session is revoked from storage

#### Scenario: Session within the absolute lifetime

- **GIVEN** a session whose age is below the absolute lifetime and whose idle window is fresh
- **WHEN** an authenticated request is made
- **THEN** the request is processed normally

### Requirement: Session expires after inactivity

The server SHALL reject authenticated requests once the time since the last recorded user
activity exceeds the idle timeout, revoke the session, and respond with HTTP 401
Unauthorized.

#### Scenario: Idle beyond the timeout

- **GIVEN** a session whose last recorded activity is older than the idle timeout
- **AND** the session is within the absolute lifetime
- **WHEN** an authenticated request is made
- **THEN** the server responds with HTTP 401 Unauthorized
- **AND** the session is revoked from storage

#### Scenario: Activity within the timeout

- **GIVEN** a session whose last recorded activity is within the idle timeout
- **WHEN** an authenticated request is made
- **THEN** the request is processed normally

### Requirement: Idle timeout counts only explicit user activity

Only an explicit, authenticated activity signal SHALL update the last-activity time.
Ordinary authenticated requests, including periodic background polling, SHALL NOT update it
and SHALL NOT extend the session.

#### Scenario: Activity signal resets the idle window

- **GIVEN** a session within both limits
- **WHEN** the client sends an authenticated activity signal
- **THEN** the session's last-activity time is updated to the current time

#### Scenario: Background polling does not extend the session

- **GIVEN** a session whose last recorded activity is older than the idle timeout
- **WHEN** the client makes a periodic background polling request to a protected endpoint
- **THEN** the request is rejected with HTTP 401 Unauthorized
- **AND** the session is not kept alive by the request

### Requirement: Session timeouts are configurable and server-authoritative

The idle timeout and the absolute lifetime SHALL be configurable through environment
variables, defaulting to 1800 seconds and 14400 seconds respectively. The server SHALL be
the authority on both limits; client-side checks SHALL NOT override a server decision.

#### Scenario: Defaults are applied when unset

- **GIVEN** the timeout environment variables are not set
- **WHEN** the server starts
- **THEN** the idle timeout is 1800 seconds and the absolute lifetime is 14400 seconds

#### Scenario: Configured values override the defaults

- **GIVEN** the timeout environment variables are set
- **WHEN** the server starts
- **THEN** the configured values are used for enforcement

#### Scenario: Server decision wins over a stale client

- **GIVEN** a client that believes its session is still valid
- **AND** a server session that has exceeded a timeout
- **WHEN** the client makes an authenticated request
- **THEN** the server rejects it with HTTP 401 Unauthorized

### Requirement: Client ends the session proactively on timeout

The client SHALL end the local session — clear stored tokens and navigate to the login page
— when it can determine that the idle timeout or the absolute lifetime has passed, without
waiting for a background request to fail.

#### Scenario: Idle timeout reached locally

- **GIVEN** an authenticated session with no user activity for longer than the idle timeout
- **WHEN** the client's periodic check runs
- **THEN** the client clears the stored tokens and navigates to the login page

#### Scenario: Absolute lifetime reached locally

- **GIVEN** an authenticated session whose locally-tracked start is older than the absolute lifetime
- **WHEN** the client's periodic check runs
- **THEN** the client clears the stored tokens and navigates to the login page

### Requirement: Timed-out sessions are revoked

When the server rejects a request or a refresh because of a timeout, it SHALL delete the
session's stored refresh token so the session cannot be resumed.

#### Scenario: Timeout revokes the refresh token

- **GIVEN** a session that has exceeded a timeout and has a stored refresh token
- **WHEN** the server rejects the request with HTTP 401 Unauthorized
- **THEN** the stored refresh token is removed from storage
