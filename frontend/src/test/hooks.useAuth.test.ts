import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useAuth } from "../hooks/useAuth";

beforeEach(() => {
  sessionStorage.clear();
  localStorage.clear();
  vi.restoreAllMocks();
});

function mockFetch(status: number, body: unknown) {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
    text: () => Promise.resolve(typeof body === "string" ? body : JSON.stringify(body)),
  });
}

describe("useAuth", () => {
  it("returns loading=true initially, then user", async () => {
    sessionStorage.setItem("populatrs_token", "test-token");
    mockFetch(200, { sub: "user-1", email: "test@test.com", name: "Test User" });

    const { result } = renderHook(() => useAuth());

    // Initially loading
    expect(result.current.loading).toBe(true);
    expect(result.current.user).toBeNull();

    // After fetch resolves
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.user).toEqual({
      sub: "user-1",
      email: "test@test.com",
      name: "Test User",
    });
  });

  it("returns null user when no token", async () => {
    const { result } = renderHook(() => useAuth());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.user).toBeNull();
  });

  it("returns null user on fetch error", async () => {
    sessionStorage.setItem("populatrs_token", "invalid-token");
    mockFetch(401, "Unauthorized");

    const { result } = renderHook(() => useAuth());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.user).toBeNull();
    // Token should be cleared on error
    expect(sessionStorage.getItem("populatrs_token")).toBeNull();
  });

  it("uses token from localStorage when sessionStorage is empty", async () => {
    localStorage.setItem("populatrs_token", "local-token");
    mockFetch(200, { sub: "local-user", email: "local@test.com", name: "Local" });

    const { result } = renderHook(() => useAuth());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.user?.email).toBe("local@test.com");
  });

  it("sets error=true on a network failure and keeps the token", async () => {
    sessionStorage.setItem("populatrs_token", "valid-token");
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("Network failure"));

    const { result } = renderHook(() => useAuth());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.user).toBeNull();
    expect(result.current.error).toBe(true);
    // A transient failure must NOT clear the token.
    expect(sessionStorage.getItem("populatrs_token")).toBe("valid-token");
  });

  it("sets error=false when the session is expired (SessionExpiredError)", async () => {
    sessionStorage.setItem("populatrs_token", "expired-token");
    mockFetch(401, "Unauthorized");

    const { result } = renderHook(() => useAuth());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.user).toBeNull();
    expect(result.current.error).toBe(false);
    expect(sessionStorage.getItem("populatrs_token")).toBeNull();
  });

  it("retry() re-runs the check and recovers after a transient failure", async () => {
    sessionStorage.setItem("populatrs_token", "valid-token");
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("Network failure"));

    const { result } = renderHook(() => useAuth());
    await waitFor(() => expect(result.current.error).toBe(true));

    // Backend becomes reachable.
    mockFetch(200, { sub: "u", email: "recovered@test.com", name: "Recovered" });
    act(() => result.current.retry());

    await waitFor(() => expect(result.current.user?.email).toBe("recovered@test.com"));
    expect(result.current.error).toBe(false);
  });
});