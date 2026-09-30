# auth/session-lifecycle Specification

## Purpose
Defines how the application initiates OIDC login, validates and stores access tokens,
silently refreshes them, and terminates the session — ensuring login always reaches the
identity provider when no valid session exists and that sessions can actually end.

## Requirements

### Requirement: Login initiation requires a valid session

When the user initiates login, the application SHALL redirect to the OIDC provider
unless a non-expired access token is already present. An expired or malformed token
SHALL NOT be treated as an active session.

#### Scenario: No token present

- **GIVEN** no access token exists in browser storage
- **WHEN** the user opens the login page and clicks "Iniciar con OIDC"
- **THEN** the browser is redirected to the OIDC provider's authorization endpoint
- **AND** the request includes `state`, `code_challenge`, and `code_challenge_method=S256`

#### Scenario: Expired token present

- **GIVEN** an access token whose `exp` claim is in the past exists in browser storage
- **WHEN** the user opens the login page
- **THEN** the stale token is removed from storage
- **AND** the login button is shown instead of an automatic redirect into the app

#### Scenario: Valid token present

- **GIVEN** a non-expired access token exists in browser storage
- **WHEN** the user opens the login page
- **THEN** the application navigates to the dashboard without contacting the OIDC provider

### Requirement: Session validity is determined client-side

The client SHALL expose a session-validity check based on the JWT `exp` claim, allowing
a small clock-skew tolerance. A token that is expired or cannot be decoded SHALL be
reported as invalid. The login decision SHALL use this check.

#### Scenario: Expired token is reported invalid

- **GIVEN** a stored JWT whose `exp` is earlier than the current time minus the skew allowance
- **WHEN** the client checks session validity
- **THEN** the check reports the session as invalid

#### Scenario: Valid token is reported valid

- **GIVEN** a stored JWT whose `exp` is later than the current time plus the skew allowance
- **WHEN** the client checks session validity
- **THEN** the check reports the session as valid

#### Scenario: Malformed token is reported invalid

- **GIVEN** a stored value that is not a well-formed JWT
- **WHEN** the client checks session validity
- **THEN** the check reports the session as invalid

### Requirement: Silent refresh works with an expired access token

The client SHALL keep the raw stored access token retrievable independently of the
validity check, so the refresh flow can present an expired token to the server, which
validates its signature while ignoring expiration.

#### Scenario: Refresh uses an expired access token

- **GIVEN** an expired access token is stored and a valid refresh token exists server-side
- **WHEN** an API call returns HTTP 401 and the client attempts a silent refresh
- **THEN** the client sends the expired access token to `POST /auth/refresh`
- **AND** the server returns a new access token

### Requirement: Access token is scoped to the browser session

The client SHALL persist the access token in `sessionStorage` only. It SHALL NOT write
the access token to `localStorage`, so that closing the browser ends the session.

#### Scenario: Token is not persisted across browser restarts

- **GIVEN** a user has logged in successfully
- **WHEN** the browser is closed and reopened
- **THEN** no access token is available in `localStorage`
- **AND** the user must authenticate again

#### Scenario: Refresh does not leak token to localStorage

- **GIVEN** an access token has expired and a silent refresh succeeds
- **WHEN** the new access token is stored
- **THEN** it is written to `sessionStorage`
- **AND** `localStorage` contains no access token

### Requirement: Explicit logout terminates the session

Logout SHALL clear all client-side tokens and SHALL revoke the server-side refresh token
for the authenticated user. When the OIDC provider advertises an `end_session_endpoint`,
the client SHALL be redirected there to end the provider SSO session.

#### Scenario: Logout revokes server-side refresh token

- **GIVEN** an authenticated user with a stored refresh token
- **WHEN** the user clicks "Cerrar sesión"
- **THEN** the client calls `POST /auth/logout` with the bearer token
- **AND** the server deletes the user's refresh token from storage
- **AND** the client clears `sessionStorage` and `localStorage`

#### Scenario: Logout works with an expired access token

- **GIVEN** the stored access token has expired but a refresh token exists server-side
- **WHEN** the user clicks "Cerrar sesión"
- **THEN** the server still identifies the user and deletes the refresh token
- **AND** the client clears local tokens

#### Scenario: Logout ends provider SSO session

- **GIVEN** the OIDC discovery document advertises an `end_session_endpoint`
- **WHEN** the user logs out
- **THEN** the browser is redirected to the provider's `end_session_endpoint`

#### Scenario: Logout without provider end-session support

- **GIVEN** the OIDC discovery document has no `end_session_endpoint`
- **WHEN** the user logs out
- **THEN** the client clears local tokens and navigates to the login page
- **AND** no redirect to the provider occurs

### Requirement: Refresh tokens expire

The server SHALL reject refresh attempts when the stored refresh token has passed its
`expires_at`. Expired refresh tokens SHALL NOT be returned by storage lookups.

#### Scenario: Expired refresh token is rejected

- **GIVEN** a stored refresh token whose `expires_at` is in the past
- **WHEN** the client calls `POST /auth/refresh`
- **THEN** the server responds with HTTP 401 Unauthorized
- **AND** the expired refresh token is removed from storage

#### Scenario: Valid refresh token is accepted

- **GIVEN** a stored refresh token whose `expires_at` is in the future
- **WHEN** the client calls `POST /auth/refresh`
- **THEN** the server exchanges it with the provider and returns a new access token

### Requirement: Login redirect does not loop on transient failures

When the application cannot verify the session because of a transient failure (network
error or server error), it SHALL NOT bounce indefinitely between the login page and the
protected area. It SHALL surface an error state that lets the user retry.

#### Scenario: Network error with a locally-valid token

- **GIVEN** a non-expired access token is stored
- **AND** the backend is unreachable
- **WHEN** the user opens the protected area
- **THEN** the application shows an error state with a retry action
- **AND** it does not bounce between `/` and `/login`

#### Scenario: Retry after transient failure

- **GIVEN** the error state is shown
- **WHEN** the user retries and the backend is reachable
- **THEN** the session is verified and the protected area is shown

### Requirement: Logout redirect target is validated

The client SHALL only navigate to an `end_session_url` whose scheme is `http` or `https`.
Any other value, including an empty string, SHALL fall back to the login page.

#### Scenario: Non-http(s) end-session URL

- **GIVEN** the logout response contains an `end_session_url` with a non-http(s) scheme
- **WHEN** the client processes logout
- **THEN** it navigates to `/login` instead

#### Scenario: Empty end-session URL

- **GIVEN** the logout response contains an empty `end_session_url`
- **WHEN** the client processes logout
- **THEN** it navigates to `/login`

### Requirement: OIDC discovery exposes end-session endpoint

The server SHALL parse the `end_session_endpoint` from the OIDC discovery document when
present, and SHALL expose it to the client for RP-initiated logout.

#### Scenario: Discovery includes end-session endpoint

- **GIVEN** the provider's discovery document contains `end_session_endpoint`
- **WHEN** the server loads OIDC metadata
- **THEN** the endpoint is available for the logout flow

#### Scenario: Discovery omits end-session endpoint

- **GIVEN** the provider's discovery document does not contain `end_session_endpoint`
- **WHEN** the server loads OIDC metadata
- **THEN** the server starts normally and logout falls back to local token clearing

#### Scenario: Malformed end-session endpoint is ignored

- **GIVEN** the discovery document contains an `end_session_endpoint` that is not a valid absolute `http`/`https` URL
- **WHEN** the server builds the logout URL
- **THEN** it returns no end-session URL and the client falls back to `/login`

### Requirement: Logout invalidates in-flight refreshes

The client SHALL ensure that a silent refresh which completes after the session has been
cleared does not restore the access token. Once logout clears the session, any in-flight
refresh SHALL be discarded and SHALL NOT write a token to storage.

#### Scenario: Refresh resolves after logout

- **GIVEN** a silent refresh is in flight
- **WHEN** the user logs out and the session is cleared
- **THEN** the in-flight refresh does not store a new access token
- **AND** the user remains on the login page

#### Scenario: Refresh resolves before logout

- **GIVEN** a silent refresh is in flight
- **WHEN** the refresh completes before the session is cleared
- **THEN** the new access token is stored normally

### Requirement: Refresh rotation cannot resurrect a revoked session

The server SHALL only persist a rotated refresh token when the stored token is still the
one used for the exchange. If the stored token was removed or replaced concurrently (for
example by logout), the server SHALL NOT persist the rotated token and SHALL reject the
refresh request.

#### Scenario: Logout during refresh exchange

- **GIVEN** a refresh token is stored for the user
- **WHEN** a refresh exchange is in flight and logout removes the stored token
- **THEN** the rotated refresh token is not persisted
- **AND** the refresh request is rejected with HTTP 401 Unauthorized

#### Scenario: Normal rotation

- **GIVEN** a refresh token is stored for the user
- **WHEN** a refresh exchange completes and the stored token is unchanged
- **THEN** the rotated refresh token is persisted
