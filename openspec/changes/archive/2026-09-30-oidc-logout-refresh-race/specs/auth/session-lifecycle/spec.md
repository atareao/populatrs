# Spec Delta

## ADDED Requirements

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
