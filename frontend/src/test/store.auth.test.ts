import { describe, it, expect, beforeEach } from "vitest";
import { getToken, setToken, clearToken } from "../store/auth";

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

  it("stores in both sessionStorage and localStorage", () => {
    setToken("token-dual");
    expect(sessionStorage.getItem("populatrs_token")).toBe("token-dual");
    expect(localStorage.getItem("populatrs_token")).toBe("token-dual");
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
});