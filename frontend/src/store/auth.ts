const TOKEN_KEY = "populatrs_token";
const SESSION_START_KEY = "populatrs_session_start";
const LAST_ACTIVITY_KEY = "populatrs_last_activity";

// Allow a small clock-skew tolerance so a token that just expired is not
// rejected while the server would still accept it.
const CLOCK_SKEW_SECONDS = 30;

// Client-side timeout mirrors of the backend defaults (D5). They may be
// hardcoded here because the server remains authoritative; these are used only
// for best-effort proactive expiry (D6/D7).
export const SESSION_IDLE_TIMEOUT_SECONDS = 30 * 60; // 30 minutes
export const SESSION_ABSOLUTE_TIMEOUT_SECONDS = 4 * 60 * 60; // 4 hours

// Effective limits used for proactive expiry. They default to the constants
// above and are overridden with the server-configured values (returned by
// `GET /api/me`) once known, so an operator raising the server limits never
// causes the client to log the user out prematurely. The server stays
// authoritative.
let effectiveIdleSeconds = SESSION_IDLE_TIMEOUT_SECONDS;
let effectiveAbsoluteSeconds = SESSION_ABSOLUTE_TIMEOUT_SECONDS;

function isPositiveFinite(value: number | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

/**
 * Overrides the effective session limits with the values advertised by the
 * server. A missing or invalid (non-finite / non-positive) value is ignored so
 * the previously configured limit (or the built-in default) is preserved.
 */
export function setSessionLimits(idle?: number, absolute?: number): void {
  if (isPositiveFinite(idle)) effectiveIdleSeconds = idle;
  if (isPositiveFinite(absolute)) effectiveAbsoluteSeconds = absolute;
}

/** Returns the effective idle limit in seconds. */
export function getSessionIdleLimit(): number {
  return effectiveIdleSeconds;
}

/** Returns the effective absolute limit in seconds. */
export function getSessionAbsoluteLimit(): number {
  return effectiveAbsoluteSeconds;
}

/** Restores the effective limits to the built-in defaults (test helper). */
export function resetSessionLimits(): void {
  effectiveIdleSeconds = SESSION_IDLE_TIMEOUT_SECONDS;
  effectiveAbsoluteSeconds = SESSION_ABSOLUTE_TIMEOUT_SECONDS;
}

// Monotonic counter bumped every time the session is cleared. Async flows that
// outlive a logout (e.g. an in-flight silent refresh) capture this value before
// awaiting and discard their result if it changed, so a late response cannot
// resurrect a session the user already ended.
let sessionGeneration = 0;

/** Returns the current session generation. See `clearToken`. */
export function getSessionGeneration(): number {
  return sessionGeneration;
}

/**
 * Returns the RAW stored access token, without filtering by expiry.
 *
 * The silent-refresh flow needs the expired token to present it to
 * `POST /auth/refresh` (the server validates the signature while ignoring
 * expiration), so this must never return `null` just because `exp` passed.
 *
 * Reads `sessionStorage` first, then falls back to `localStorage` so legacy
 * tokens written by older builds can be picked up and cleaned.
 */
export function getToken(): string | null {
  try {
    return sessionStorage.getItem(TOKEN_KEY) || localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

/**
 * Persists the access token in `sessionStorage` ONLY, so closing the browser
 * ends the session. Never writes to `localStorage`.
 */
export function setToken(token: string): void {
  try {
    sessionStorage.setItem(TOKEN_KEY, token);
  } catch { /* noop */ }
}

/** Removes the token from both stores (purges legacy `localStorage` entries). */
export function clearToken(): void {
  // Invalidate any async continuation that started before this logout.
  sessionGeneration += 1;
  try {
    sessionStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(TOKEN_KEY);
    sessionStorage.removeItem(SESSION_START_KEY);
    sessionStorage.removeItem(LAST_ACTIVITY_KEY);
  } catch { /* noop */ }
}

function readTimestamp(key: string): number | null {
  try {
    const raw = sessionStorage.getItem(key);
    if (raw === null) return null;
    const value = Number(raw);
    return Number.isFinite(value) ? value : null;
  } catch {
    return null;
  }
}

function writeTimestamp(key: string, value: number): void {
  try {
    sessionStorage.setItem(key, String(value));
  } catch { /* noop */ }
}

/**
 * Records the start of a session (login) and seeds the last-activity time.
 * The start is the immutable anchor for the absolute lifetime; it is only ever
 * set by this function (or cleared), never moved by activity or refresh.
 */
export function startSession(now: number = Date.now()): void {
  writeTimestamp(SESSION_START_KEY, now);
  writeTimestamp(LAST_ACTIVITY_KEY, now);
}

/** Returns the recorded session start (epoch ms), or `null` when unknown. */
export function getSessionStart(): number | null {
  return readTimestamp(SESSION_START_KEY);
}

/** Returns the recorded last-activity time (epoch ms), or `null` when unknown. */
export function getLastActivity(): number | null {
  return readTimestamp(LAST_ACTIVITY_KEY);
}

/**
 * Records a genuine user-activity signal, moving only the last-activity time.
 * Never touches the session start, so the absolute lifetime is preserved.
 */
export function recordActivity(now: number = Date.now()): void {
  writeTimestamp(LAST_ACTIVITY_KEY, now);
}

/**
 * Seeds the session window from the first observation when a token exists but
 * no window was recorded. This covers sessions whose token was injected by the
 * server (OIDC callback / dev-login HTML) without a frontend login page, so the
 * absolute lifetime still has an anchor. Existing windows are never moved.
 */
export function ensureSessionWindow(now: number = Date.now()): void {
  if (getSessionStart() === null) {
    startSession(now);
  }
}

/**
 * Reports whether the locally-tracked session window is still open, allowing
 * the same clock-skew tolerance as the token check. An untracked window is
 * treated as valid (the server stays authoritative).
 */
export function isSessionWindowValid(now: number = Date.now()): boolean {
  const start = getSessionStart();
  if (start !== null && now - start > (effectiveAbsoluteSeconds + CLOCK_SKEW_SECONDS) * 1000) {
    return false;
  }

  const lastActivity = getLastActivity();
  if (lastActivity !== null && now - lastActivity > (effectiveIdleSeconds + CLOCK_SKEW_SECONDS) * 1000) {
    return false;
  }

  return true;
}

function decodeJwtPayload(token: string): Record<string, unknown> | null {
  try {
    const parts = token.split(".");
    if (parts.length !== 3) return null;

    const base64 = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const padding = (4 - (base64.length % 4)) % 4;
    const json = atob(base64 + "=".repeat(padding));
    const payload: unknown = JSON.parse(json);

    if (payload && typeof payload === "object") {
      return payload as Record<string, unknown>;
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * Decodes the JWT payload and reports whether the token is still valid,
 * allowing a small clock-skew tolerance. Any decode/parse failure (malformed
 * token, missing `exp`, non-numeric `exp`) is reported as invalid.
 */
export function isTokenValid(token: string | null): boolean {
  if (!token) return false;

  const payload = decodeJwtPayload(token);
  if (!payload) return false;

  const exp = payload.exp;
  if (typeof exp !== "number") return false;

  return exp > Date.now() / 1000 - CLOCK_SKEW_SECONDS;
}

/**
 * True only when a stored token exists, is not expired, AND the locally-tracked
 * session window (idle + absolute) has not passed. The check is best-effort;
 * the server remains authoritative.
 *
 * This is a pure predicate: it never anchors the window. The window is anchored
 * at real login time (server-rendered HTML) or, for legacy tokens, by the
 * explicit `ensureSessionWindow` fallback called from the activity hook.
 */
export function hasValidSession(now: number = Date.now()): boolean {
  if (!isTokenValid(getToken())) return false;

  return isSessionWindowValid(now);
}
