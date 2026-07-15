import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
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
});