import { describe, it, expect, beforeEach } from "vitest";
import {
  getToken,
  setToken,
  clearToken,
  isTokenValid,
  hasValidSession,
  getSessionGeneration,
} from "../store/auth";

// --- JWT helpers -----------------------------------------------------------
// Build unsigned JWTs (header.payload.) with a base64url-encoded payload.
// No signature is needed: the client only decodes the payload to read `exp`.
function base64url(input: string): string {
  return btoa(input).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function makeJwt(payload: Record<string, unknown>): string {
  const header = base64url(JSON.stringify({ alg: "none", typ: "JWT" }));
  const body = base64url(JSON.stringify(payload));
  return `${header}.${body}.`;
}

const nowSeconds = () => Math.floor(Date.now() / 1000);

describe("store/auth", () => {
  beforeEach(() => {
    sessionStorage.clear();
    localStorage.clear();
  });

  it("returns null when no token is stored", () => {
    expect(getToken()).toBeNull();
  });

  it("stores and retrieves a token", () => {
    setToken("test-token-123");
    expect(getToken()).toBe("test-token-123");
  });

  it("stores in sessionStorage only", () => {
    setToken("token-session");
    expect(sessionStorage.getItem("populatrs_token")).toBe("token-session");
    expect(localStorage.getItem("populatrs_token")).toBeNull();
  });

  it("clears the token", () => {
    setToken("to-clear");
    clearToken();
    expect(getToken()).toBeNull();
  });

  it("clears from both storage backends", () => {
    setToken("to-clear");
    clearToken();
    expect(sessionStorage.getItem("populatrs_token")).toBeNull();
    expect(localStorage.getItem("populatrs_token")).toBeNull();
  });

  describe("getSessionGeneration", () => {
    it("increments the session generation when the token is cleared", () => {
      const before = getSessionGeneration();
      clearToken();
      expect(getSessionGeneration()).toBe(before + 1);
    });

    it("increments on every clear, even when no token is stored", () => {
      const before = getSessionGeneration();
      clearToken();
      clearToken();
      expect(getSessionGeneration()).toBe(before + 2);
    });
  });

  it("prefers sessionStorage over localStorage", () => {
    localStorage.setItem("populatrs_token", "local-token");
    sessionStorage.setItem("populatrs_token", "session-token");
    expect(getToken()).toBe("session-token");
  });

  it("falls back to localStorage when sessionStorage is empty", () => {
    localStorage.setItem("populatrs_token", "local-only");
    expect(getToken()).toBe("local-only");
  });

  it("handles storage errors gracefully", () => {
    const originalSetItem = Storage.prototype.setItem;
    Storage.prototype.setItem = () => {
      throw new Error("Storage full");
    };

    // Should not throw
    expect(() => setToken("token")).not.toThrow();
    expect(() => clearToken()).not.toThrow();
    expect(() => getToken()).not.toThrow();

    Storage.prototype.setItem = originalSetItem;
  });

  describe("isTokenValid", () => {
    it("returns true for a JWT whose exp is in the future", () => {
      expect(isTokenValid(makeJwt({ exp: nowSeconds() + 3600 }))).toBe(true);
    });

    it("returns false for a JWT whose exp is in the past", () => {
      expect(isTokenValid(makeJwt({ exp: nowSeconds() - 3600 }))).toBe(false);
    });

    it("returns false for a malformed token", () => {
      expect(isTokenValid("not-a-jwt")).toBe(false);
    });

    it("returns false for null", () => {
      expect(isTokenValid(null)).toBe(false);
    });

    it("returns false for an empty string", () => {
      expect(isTokenValid("")).toBe(false);
    });

    it("returns false when the payload has no exp claim", () => {
      expect(isTokenValid(makeJwt({ sub: "user" }))).toBe(false);
    });

    it("treats a token expiring within the skew window as valid", () => {
      expect(isTokenValid(makeJwt({ exp: nowSeconds() - 10 }))).toBe(true);
    });

    it("treats a token expired beyond the skew window as invalid", () => {
      expect(isTokenValid(makeJwt({ exp: nowSeconds() - 60 }))).toBe(false);
    });

    it("treats a token expiring shortly in the future as valid", () => {
      expect(isTokenValid(makeJwt({ exp: nowSeconds() + 10 }))).toBe(true);
    });

    it("returns false for a token with only two segments", () => {
      const twoSegments = makeJwt({ exp: nowSeconds() + 3600 }).replace(/\.$/, "");
      expect(isTokenValid(twoSegments)).toBe(false);
    });

    it("returns false for a token with four segments", () => {
      const fourSegments = makeJwt({ exp: nowSeconds() + 3600 }) + "sig.extra";
      expect(isTokenValid(fourSegments)).toBe(false);
    });

    it("decodes a payload whose standard base64 contains + and /", () => {
      const plusPayload = { exp: nowSeconds() + 3600, pad: " >" };
      const slashPayload = { exp: nowSeconds() + 3600, pad: " ?" };

      // Sanity: these payloads really exercise the base64url conversion.
      expect(btoa(JSON.stringify(plusPayload))).toContain("+");
      expect(btoa(JSON.stringify(slashPayload))).toContain("/");

      expect(isTokenValid(makeJwt(plusPayload))).toBe(true);
      expect(isTokenValid(makeJwt(slashPayload))).toBe(true);
    });
  });

  describe("hasValidSession", () => {
    it("returns true only when a valid token is stored", () => {
      setToken(makeJwt({ exp: nowSeconds() + 3600 }));
      expect(hasValidSession()).toBe(true);
    });

    it("returns false when the stored token is expired", () => {
      setToken(makeJwt({ exp: nowSeconds() - 3600 }));
      expect(hasValidSession()).toBe(false);
    });

    it("returns false when no token is stored", () => {
      expect(hasValidSession()).toBe(false);
    });
  });

  describe("getToken", () => {
    it("returns the raw token even when it is expired", () => {
      const expired = makeJwt({ exp: nowSeconds() - 3600 });
      setToken(expired);
      expect(getToken()).toBe(expired);
    });
  });

  describe("setToken", () => {
    it("writes to sessionStorage and not to localStorage", () => {
      setToken("session-only");
      expect(sessionStorage.getItem("populatrs_token")).toBe("session-only");
      expect(localStorage.getItem("populatrs_token")).toBeNull();
    });
  });
});