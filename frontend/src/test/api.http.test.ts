import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  fetchMe,
  fetchFeeds,
  createFeed,
  updateFeed,
  deleteFeed,
  toggleFeed,
  fetchPublishers,
  updatePublisher,
  fetchSchedule,
  updateSchedule,
  fetchStorage,
  updateStorage,
  fetchStatus,
  logout,
  sendActivity,
  SessionExpiredError,
  type FeedConfig,
} from "../api/http";
import { clearToken } from "../store/auth";

// Mock sessionStorage
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

function mockFetchError(message: string) {
  globalThis.fetch = vi.fn().mockRejectedValue(new Error(message));
}

// URL-aware mock: lets a test return different responses per endpoint.
function mockFetchByUrl(
  responses: Record<string, { status: number; body: unknown }>,
) {
  globalThis.fetch = vi.fn().mockImplementation((url: string) => {
    const entry = responses[url] ?? { status: 500, body: "unexpected url" };
    return Promise.resolve({
      ok: entry.status >= 200 && entry.status < 300,
      status: entry.status,
      json: () => Promise.resolve(entry.body),
      text: () =>
        Promise.resolve(
          typeof entry.body === "string" ? entry.body : JSON.stringify(entry.body),
        ),
    });
  });
}

function setToken(token: string) {
  sessionStorage.setItem("populatrs_token", token);
}

// A promise whose resolution is controlled by the test, so a silent refresh
// can be held "in flight" while the test clears the session.
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

// Flush pending microtasks and timers so the 401 interceptor has started the
// refresh before the test proceeds.
function flushAsync(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("api/http", () => {
  describe("fetchMe", () => {
    it("returns user when authenticated", async () => {
      setToken("valid-token");
      mockFetch(200, { sub: "user-1", email: "test@test.com", name: "Test" });

      const user = await fetchMe();
      expect(user.email).toBe("test@test.com");
      expect(user.name).toBe("Test");
      expect(fetch).toHaveBeenCalledWith("/api/me", expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer valid-token" }),
      }));
    });

    it("throws on non-ok response", async () => {
      setToken("t");
      mockFetch(500, "Server Error");

      await expect(fetchMe()).rejects.toThrow("HTTP 500: Server Error");
    });

    it("throws 'Session expired' and clears the token when refresh fails on 401", async () => {
      setToken("t");
      // Both the original request and the refresh attempt return 401.
      mockFetch(401, "Unauthorized");

      await expect(fetchMe()).rejects.toThrow("Session expired");
      expect(sessionStorage.getItem("populatrs_token")).toBeNull();
    });

    it("throws a SessionExpiredError when refresh fails on 401", async () => {
      setToken("t");
      mockFetch(401, "Unauthorized");

      const err = await fetchMe().catch((e: unknown) => e);
      expect(err).toBeInstanceOf(SessionExpiredError);
    });

    it("throws a plain Error (not SessionExpiredError) on a network failure", async () => {
      setToken("t");
      mockFetchError("Network failure");

      const err = await fetchMe().catch((e: unknown) => e);
      expect(err).toBeInstanceOf(Error);
      expect(err).not.toBeInstanceOf(SessionExpiredError);
    });

    it("keeps the token and throws a plain Error when refresh returns 500", async () => {
      setToken("t");
      mockFetchByUrl({
        "/api/me": { status: 401, body: "Unauthorized" },
        "/auth/refresh": { status: 500, body: "Server Error" },
      });

      const err = await fetchMe().catch((e: unknown) => e);
      expect(err).toBeInstanceOf(Error);
      expect(err).not.toBeInstanceOf(SessionExpiredError);
      expect(sessionStorage.getItem("populatrs_token")).toBe("t");
    });

    it("keeps the token when the refresh request fails with a network error", async () => {
      setToken("t");
      globalThis.fetch = vi.fn().mockImplementation((url: string) => {
        if (url === "/auth/refresh") {
          return Promise.reject(new Error("Network failure"));
        }
        return Promise.resolve({
          ok: false,
          status: 401,
          json: () => Promise.resolve("Unauthorized"),
          text: () => Promise.resolve("Unauthorized"),
        });
      });

      const err = await fetchMe().catch((e: unknown) => e);
      expect(err).not.toBeInstanceOf(SessionExpiredError);
      expect(sessionStorage.getItem("populatrs_token")).toBe("t");
    });

    it("clears the token and throws SessionExpiredError when refresh returns 401", async () => {
      setToken("t");
      mockFetchByUrl({
        "/api/me": { status: 401, body: "Unauthorized" },
        "/auth/refresh": { status: 401, body: "Unauthorized" },
      });

      const err = await fetchMe().catch((e: unknown) => e);
      expect(err).toBeInstanceOf(SessionExpiredError);
      expect(sessionStorage.getItem("populatrs_token")).toBeNull();
    });

    it("keeps the refreshed token and throws a plain Error when the retried request returns 500", async () => {
      setToken("t");
      let meCalls = 0;
      globalThis.fetch = vi.fn().mockImplementation((url: string) => {
        if (url === "/auth/refresh") {
          return Promise.resolve({
            ok: true,
            status: 200,
            json: () => Promise.resolve({ access_token: "new-token" }),
            text: () => Promise.resolve(""),
          });
        }
        meCalls += 1;
        const status = meCalls === 1 ? 401 : 500;
        return Promise.resolve({
          ok: false,
          status,
          json: () => Promise.resolve("err"),
          text: () => Promise.resolve("Server Error"),
        });
      });

      const err = await fetchMe().catch((e: unknown) => e);
      expect(err).toBeInstanceOf(Error);
      expect(err).not.toBeInstanceOf(SessionExpiredError);
      expect(sessionStorage.getItem("populatrs_token")).toBe("new-token");
    });

    it("clears the token and throws SessionExpiredError when the retried request returns 401", async () => {
      setToken("t");
      globalThis.fetch = vi.fn().mockImplementation((url: string) => {
        if (url === "/auth/refresh") {
          return Promise.resolve({
            ok: true,
            status: 200,
            json: () => Promise.resolve({ access_token: "new-token" }),
            text: () => Promise.resolve(""),
          });
        }
        return Promise.resolve({
          ok: false,
          status: 401,
          json: () => Promise.resolve("Unauthorized"),
          text: () => Promise.resolve("Unauthorized"),
        });
      });

      const err = await fetchMe().catch((e: unknown) => e);
      expect(err).toBeInstanceOf(SessionExpiredError);
      expect(sessionStorage.getItem("populatrs_token")).toBeNull();
    });
  });

  describe("silent refresh vs. logout race", () => {
    it("discards a silent refresh that resolves after the session is cleared", async () => {
      setToken("t");
      const refresh = deferred<{ access_token: string }>();
      let meCalls = 0;
      globalThis.fetch = vi.fn().mockImplementation((url: string) => {
        if (url === "/auth/refresh") {
          return refresh.promise.then((body) => ({
            ok: true,
            status: 200,
            json: () => Promise.resolve(body),
            text: () => Promise.resolve(""),
          }));
        }
        meCalls += 1;
        if (meCalls === 1) {
          return Promise.resolve({
            ok: false,
            status: 401,
            json: () => Promise.resolve("Unauthorized"),
            text: () => Promise.resolve("Unauthorized"),
          });
        }
        // The retry would succeed if the resurrected token were stored.
        return Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve({ sub: "u", email: "e", name: "n" }),
          text: () => Promise.resolve(""),
        });
      });

      const pending = fetchMe().catch((e: unknown) => e);
      // Let the 401 interceptor start the refresh, then log out mid-flight.
      await flushAsync();
      clearToken();
      refresh.resolve({ access_token: "resurrected-token" });

      const err = await pending;
      expect(err).toBeInstanceOf(SessionExpiredError);
      expect(sessionStorage.getItem("populatrs_token")).toBeNull();
    });

    it("stores the new token when the silent refresh resolves before the session is cleared", async () => {
      setToken("t");
      const refresh = deferred<{ access_token: string }>();
      let meCalls = 0;
      globalThis.fetch = vi.fn().mockImplementation((url: string) => {
        if (url === "/auth/refresh") {
          return refresh.promise.then((body) => ({
            ok: true,
            status: 200,
            json: () => Promise.resolve(body),
            text: () => Promise.resolve(""),
          }));
        }
        meCalls += 1;
        if (meCalls === 1) {
          return Promise.resolve({
            ok: false,
            status: 401,
            json: () => Promise.resolve("Unauthorized"),
            text: () => Promise.resolve("Unauthorized"),
          });
        }
        return Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve({ sub: "u", email: "e", name: "n" }),
          text: () => Promise.resolve(""),
        });
      });

      const pending = fetchMe();
      await flushAsync();
      refresh.resolve({ access_token: "new-token" });

      const user = await pending;
      expect(user.email).toBe("e");
      expect(sessionStorage.getItem("populatrs_token")).toBe("new-token");
    });
  });

  describe("fetchFeeds", () => {
    it("returns feeds list", async () => {
      setToken("t");
      const feedData = { feeds: [{ id: "f1", name: "Feed 1" }], total: 1 };
      mockFetch(200, feedData);

      const result = await fetchFeeds();
      expect(result.total).toBe(1);
      expect(result.feeds[0].name).toBe("Feed 1");
    });
  });

  describe("createFeed", () => {
    it("sends POST with feed data", async () => {
      setToken("t");
      mockFetch(204, null);

      const feed = { id: "new-feed", name: "New Feed", type: "Rss" } as unknown as FeedConfig;
      await createFeed(feed);

      expect(fetch).toHaveBeenCalledWith("/api/feeds", expect.objectContaining({
        method: "POST",
        body: expect.any(String),
      }));
      const body = JSON.parse((fetch as ReturnType<typeof vi.fn>).mock.calls[0][1].body);
      expect(body.id).toBe("new-feed");
    });
  });

  describe("updateFeed", () => {
    it("sends PUT to feed id", async () => {
      setToken("t");
      mockFetch(204, null);

      const feed = { id: "f1", name: "Updated" } as unknown as FeedConfig;
      await updateFeed("f1", feed);

      expect(fetch).toHaveBeenCalledWith("/api/feeds/f1", expect.objectContaining({
        method: "PUT",
      }));
    });
  });

  describe("deleteFeed", () => {
    it("sends DELETE to feed id", async () => {
      setToken("t");
      mockFetch(204, null);

      await deleteFeed("f1");

      expect(fetch).toHaveBeenCalledWith("/api/feeds/f1", expect.objectContaining({
        method: "DELETE",
      }));
    });
  });

  describe("toggleFeed", () => {
    it("sends PATCH and returns enabled status", async () => {
      setToken("t");
      mockFetch(200, { enabled: false });

      const result = await toggleFeed("f1");
      expect(result.enabled).toBe(false);
      expect(fetch).toHaveBeenCalledWith("/api/feeds/f1", expect.objectContaining({
        method: "PATCH",
      }));
    });
  });

  describe("fetchPublishers", () => {
    it("returns publishers", async () => {
      setToken("t");
      mockFetch(200, { publishers: { telegram: { type: "Telegram", config: {}, enabled: true } }, total: 1 });

      const data = await fetchPublishers();
      expect(data.total).toBe(1);
      expect(data.publishers.telegram.type).toBe("Telegram");
    });
  });

  describe("updatePublisher", () => {
    it("sends PUT to publisher id", async () => {
      setToken("t");
      mockFetch(204, null);

      await updatePublisher("telegram", { type: "Telegram", config: { bot_token: "abc" }, enabled: true });

      expect(fetch).toHaveBeenCalledWith("/api/publishers/telegram", expect.objectContaining({
        method: "PUT",
      }));
    });
  });

  describe("fetchSchedule", () => {
    it("returns schedule config", async () => {
      setToken("t");
      mockFetch(200, { cron_expression: "*/30 * * * *", timezone: "Europe/Madrid", next_run_at: "2026-07-26T10:30:00+00:00" });

      const schedule = await fetchSchedule();
      expect(schedule.cron_expression).toBe("*/30 * * * *");
      expect(schedule.timezone).toBe("Europe/Madrid");
    });
  });

  describe("updateSchedule", () => {
    it("sends PUT with schedule data", async () => {
      setToken("t");
      mockFetch(204, null);

      await updateSchedule({ cron_expression: "0 * * * *", timezone: "UTC", next_run_at: null });

      expect(fetch).toHaveBeenCalledWith("/api/schedule", expect.objectContaining({
        method: "PUT",
      }));
    });
  });

  describe("fetchStorage", () => {
    it("returns storage config", async () => {
      setToken("t");
      mockFetch(200, { data_dir: "/data" });

      const storage = await fetchStorage();
      expect(storage.data_dir).toBe("/data");
    });
  });

  describe("updateStorage", () => {
    it("sends PUT with storage data", async () => {
      setToken("t");
      mockFetch(204, null);

      await updateStorage({ data_dir: "/data" });

      expect(fetch).toHaveBeenCalledWith("/api/storage", expect.objectContaining({
        method: "PUT",
      }));
    });
  });

  describe("fetchStatus", () => {
    it("returns dashboard status", async () => {
      setToken("t");
      mockFetch(200, {
        feeds: { total: 5, enabled: 3, disabled: 2 },
        publishers: { total: 2 },
        schedule: { cron_expression: "0 * * * *", timezone: "UTC", next_run_at: "2026-07-26T10:00:00+00:00" },
        storage: { data_dir: "./data" },
      });

      const status = await fetchStatus();
      expect(status.feeds.total).toBe(5);
      expect(status.publishers.total).toBe(2);
    });
  });

  describe("error handling", () => {
    it("throws on network error", async () => {
      setToken("t");
      mockFetchError("Network failure");

      await expect(fetchMe()).rejects.toThrow("Network failure");
    });

    it("throws on 404", async () => {
      setToken("t");
      mockFetch(404, "Not found");

      await expect(fetchStatus()).rejects.toThrow("HTTP 404: Not found");
    });

    it("makes request without auth header when no token", async () => {
      mockFetch(200, { sub: "u", email: "e", name: "n" });

      await fetchMe();

      const headers = (fetch as ReturnType<typeof vi.fn>).mock.calls[0][1].headers;
      expect(headers.Authorization).toBeUndefined();
    });
  });

  describe("logout", () => {
    it("POSTs to /auth/logout with the raw bearer token and returns end_session_url", async () => {
      setToken("raw-token");
      mockFetch(200, { end_session_url: "https://idp.example.com/logout" });

      const result = await logout();

      expect(result.end_session_url).toBe("https://idp.example.com/logout");
      expect(fetch).toHaveBeenCalledWith(
        "/auth/logout",
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({
            Authorization: "Bearer raw-token",
          }),
        }),
      );
    });

    it("returns null end_session_url when the server omits it", async () => {
      setToken("raw-token");
      mockFetch(200, {});

      const result = await logout();
      expect(result.end_session_url).toBeNull();
    });

    it("tolerates failure and returns null end_session_url", async () => {
      setToken("raw-token");
      mockFetchError("Network failure");

      const result = await logout();
      expect(result.end_session_url).toBeNull();
    });
  });

  describe("sendActivity", () => {
    it("issues an authenticated POST /auth/activity", async () => {
      setToken("raw-token");
      mockFetch(204, null);

      await sendActivity();

      expect(fetch).toHaveBeenCalledWith(
        "/auth/activity",
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({
            Authorization: "Bearer raw-token",
          }),
        }),
      );
    });

    it("treats a 401 from the activity call as session expiry and clears the token", async () => {
      setToken("raw-token");
      // Both the activity call and the silent refresh reject with 401.
      mockFetch(401, "Unauthorized");

      const err = await sendActivity().catch((e: unknown) => e);
      expect(err).toBeInstanceOf(SessionExpiredError);
      expect(sessionStorage.getItem("populatrs_token")).toBeNull();
    });
  });
});