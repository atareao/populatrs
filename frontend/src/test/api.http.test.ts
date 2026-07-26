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
  type FeedConfig,
} from "../api/http";

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

function setToken(token: string) {
  sessionStorage.setItem("populatrs_token", token);
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
      mockFetch(401, "Unauthorized");

      await expect(fetchMe()).rejects.toThrow("HTTP 401: Unauthorized");
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
      mockFetch(200, { cron_expression: "*/30 * * * *", timezone: "Europe/Madrid" });

      const schedule = await fetchSchedule();
      expect(schedule.cron_expression).toBe("*/30 * * * *");
      expect(schedule.timezone).toBe("Europe/Madrid");
    });
  });

  describe("updateSchedule", () => {
    it("sends PUT with schedule data", async () => {
      setToken("t");
      mockFetch(204, null);

      await updateSchedule({ cron_expression: "0 * * * *", timezone: "UTC" });

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
        schedule: { cron_expression: "0 * * * *", timezone: "UTC" },
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
});