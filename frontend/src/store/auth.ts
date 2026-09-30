const TOKEN_KEY = "populatrs_token";

// Allow a small clock-skew tolerance so a token that just expired is not
// rejected while the server would still accept it.
const CLOCK_SKEW_SECONDS = 30;

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
  } catch { /* noop */ }
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

/** True only when a stored token exists AND is not expired. */
export function hasValidSession(): boolean {
  return isTokenValid(getToken());
}
