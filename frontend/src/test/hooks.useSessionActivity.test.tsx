import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent, act, waitFor } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router";
import {
  useSessionActivity,
  SESSION_CHECK_INTERVAL_MS,
} from "../hooks/useSessionActivity";
import {
  setToken,
  getToken,
  startSession,
  recordActivity,
  SESSION_IDLE_TIMEOUT_SECONDS,
  SESSION_ABSOLUTE_TIMEOUT_SECONDS,
} from "../store/auth";

// --- JWT helpers -----------------------------------------------------------
function base64url(input: string): string {
  return btoa(input).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function makeJwt(payload: Record<string, unknown>): string {
  const header = base64url(JSON.stringify({ alg: "none", typ: "JWT" }));
  const body = base64url(JSON.stringify(payload));
  return `${header}.${body}.`;
}

const nowSeconds = () => Math.floor(Date.now() / 1000);

/** Host component: mounts the tracker inside the router. */
function HookHost() {
  useSessionActivity();
  return null;
}

function renderTracker() {
  return render(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route path="/" element={<div>HOME<HookHost /></div>} />
        <Route path="/login" element={<div>LOGIN</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

function mockActivityOk() {
  const fetchMock = vi.fn().mockResolvedValue({
    ok: true,
    status: 204,
    json: () => Promise.resolve(null),
    text: () => Promise.resolve(""),
  });
  globalThis.fetch = fetchMock;
  return fetchMock;
}

function setValidToken(): void {
  setToken(makeJwt({ exp: nowSeconds() + 3600 }));
}

describe("useSessionActivity", () => {
  beforeEach(() => {
    sessionStorage.clear();
    localStorage.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
    // Restore the jsdom prototype getter for visibilityState.
    delete (document as unknown as { visibilityState?: string }).visibilityState;
  });

  it("calls the activity endpoint on genuine user interaction", () => {
    const fetchMock = mockActivityOk();
    setValidToken();

    renderTracker();
    fireEvent.keyDown(window, { key: "a" });

    expect(fetchMock).toHaveBeenCalledWith(
      "/auth/activity",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("sends activity for a pointer/mouse interaction too", () => {
    const fetchMock = mockActivityOk();
    setValidToken();

    renderTracker();
    fireEvent.mouseDown(window);

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("does not call the activity endpoint when no token is stored", () => {
    const fetchMock = mockActivityOk();

    renderTracker();
    fireEvent.keyDown(window, { key: "a" });

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("throttles repeated activity calls to at most once per 60 seconds", () => {
    const fetchMock = mockActivityOk();
    setValidToken();

    let nowMs = 1_000_000_000_000;
    vi.spyOn(Date, "now").mockImplementation(() => nowMs);

    renderTracker();

    fireEvent.keyDown(window, { key: "a" });
    expect(fetchMock).toHaveBeenCalledTimes(1);

    // 30 s later: still inside the throttle window.
    nowMs += 30_000;
    fireEvent.keyDown(window, { key: "b" });
    expect(fetchMock).toHaveBeenCalledTimes(1);

    // 61 s after the first signal: the throttle window has passed.
    nowMs += 31_000;
    fireEvent.keyDown(window, { key: "c" });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("ignores activity while the tab is hidden", () => {
    const fetchMock = mockActivityOk();
    setValidToken();

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });

    renderTracker();
    fireEvent.keyDown(window, { key: "a" });
    fireEvent.mouseDown(window);

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("ends the local session when the idle window has already passed", () => {
    mockActivityOk();
    setValidToken();
    const stale = Date.now() - (SESSION_IDLE_TIMEOUT_SECONDS + 60) * 1000;
    startSession(stale);
    recordActivity(stale);

    renderTracker();

    expect(screen.getByText("LOGIN")).toBeInTheDocument();
    expect(getToken()).toBeNull();
  });

  it("ends the local session when the absolute lifetime has already passed", () => {
    mockActivityOk();
    setValidToken();
    const now = Date.now();
    startSession(now - (SESSION_ABSOLUTE_TIMEOUT_SECONDS + 60) * 1000);
    recordActivity(now);

    renderTracker();

    expect(screen.getByText("LOGIN")).toBeInTheDocument();
    expect(getToken()).toBeNull();
  });

  it("ends the session on the periodic check when a limit passes while open", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));

    mockActivityOk();
    setToken(makeJwt({ exp: nowSeconds() + 20_000 }));
    startSession(Date.now());
    recordActivity(Date.now());

    renderTracker();
    expect(screen.getByText("HOME")).toBeInTheDocument();

    // Move past the absolute lifetime, then fire the periodic check.
    vi.setSystemTime(
      new Date(Date.now() + (SESSION_ABSOLUTE_TIMEOUT_SECONDS + 60) * 1000),
    );
    act(() => {
      vi.advanceTimersByTime(SESSION_CHECK_INTERVAL_MS);
    });

    expect(screen.getByText("LOGIN")).toBeInTheDocument();
    expect(getToken()).toBeNull();
  });

  it("suppresses activity while hidden and re-enables it when visible again", () => {
    const fetchMock = mockActivityOk();
    setValidToken();

    let visibility = "visible";
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => visibility,
    });

    let nowMs = 1_000_000_000_000;
    vi.spyOn(Date, "now").mockImplementation(() => nowMs);

    renderTracker();

    // Visible: activity is signaled.
    fireEvent.keyDown(window, { key: "a" });
    expect(fetchMock).toHaveBeenCalledTimes(1);

    // Hide the tab, then interact: no signal.
    visibility = "hidden";
    document.dispatchEvent(new Event("visibilitychange"));
    nowMs += 61_000;
    fireEvent.keyDown(window, { key: "b" });
    expect(fetchMock).toHaveBeenCalledTimes(1);

    // Show the tab again: activity is signaled once more.
    visibility = "visible";
    document.dispatchEvent(new Event("visibilitychange"));
    nowMs += 61_000;
    fireEvent.keyDown(window, { key: "c" });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("navigates to /login when sendActivity reports an expired session", async () => {
    // Both /auth/activity and /auth/refresh return 401, so the shared
    // interceptor surfaces a SessionExpiredError.
    const fetchMock = vi.fn().mockResolvedValue({
      ok: false,
      status: 401,
      json: () => Promise.resolve({}),
      text: () => Promise.resolve("unauthorized"),
    });
    globalThis.fetch = fetchMock;
    setValidToken();

    renderTracker();
    fireEvent.keyDown(window, { key: "a" });

    await waitFor(() => {
      expect(screen.getByText("LOGIN")).toBeInTheDocument();
    });
    expect(getToken()).toBeNull();
  });
});
