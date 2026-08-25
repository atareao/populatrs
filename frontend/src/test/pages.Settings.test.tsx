import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { ConfigProvider } from "antd";
import Settings from "../pages/Settings";

const mockYoutube = { api_key: "AIzaSyTest123" };
const mockSchedule = { cron_expression: "0 */2 * * *", timezone: "Europe/Madrid", next_run_at: "2026-07-26T12:00:00+00:00" };

function mockFetch(status: number, body: unknown) {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
    text: () => Promise.resolve(typeof body === "string" ? body : JSON.stringify(body)),
  });
}

function renderSettings(initialEntries = ["/settings"]) {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={initialEntries}>
        <Settings />
      </MemoryRouter>
    </ConfigProvider>,
  );
}

describe("Settings", () => {
  beforeEach(() => {
    sessionStorage.clear();
    vi.restoreAllMocks();
    sessionStorage.setItem("populatrs_token", "test-token");
    // mock both endpoints by default
    globalThis.fetch = vi.fn()
      .mockResolvedValueOnce({
        ok: true, status: 200,
        json: () => Promise.resolve(mockYoutube),
        text: () => Promise.resolve(JSON.stringify(mockYoutube)),
      })
      .mockResolvedValueOnce({
        ok: true, status: 200,
        json: () => Promise.resolve(mockSchedule),
        text: () => Promise.resolve(JSON.stringify(mockSchedule)),
      })
      .mockResolvedValueOnce({
        ok: true, status: 200,
        json: () => Promise.resolve({ max_retries: 3, base_delay_seconds: 5, max_delay_seconds: 300, backoff_multiplier: 2.0 }),
        text: () => Promise.resolve(JSON.stringify({ max_retries: 3, base_delay_seconds: 5, max_delay_seconds: 300, backoff_multiplier: 2.0 })),
      });
  });

  it("shows loading spinner initially", () => {
    globalThis.fetch = vi.fn().mockReturnValue(new Promise(() => {}));
    renderSettings();
    expect(screen.getByTestId("spinner")).toBeInTheDocument();
  });

  it("renders the settings page title", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Settings")).toBeInTheDocument();
    });
  });

  it("loads and displays schedule section", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Schedule")).toBeInTheDocument();
    });
    expect(screen.getByDisplayValue(/Europe\/Madrid/)).toBeInTheDocument();
  });

  it("loads and displays youtube config section", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("YouTube API Key")).toBeInTheDocument();
    });
  });

  it("saves youtube config on form submit", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("YouTube API Key")).toBeInTheDocument();
    });

    const saveBtn = screen.getByRole("button", { name: /save$/i });
    await userEvent.click(saveBtn);

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/youtube",
        expect.objectContaining({ method: "PUT" }),
      );
    });
  });

  it("saves schedule on form submit", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Schedule")).toBeInTheDocument();
    });

    const saveBtn = screen.getByRole("button", { name: /save schedule/i });
    await userEvent.click(saveBtn);

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/schedule",
        expect.objectContaining({ method: "PUT" }),
      );
    });
  });

  it("shows info about youtube api key", async () => {
    renderSettings();

    await waitFor(() => {
      expect(
        screen.getByText(/YouTube Data API v3 key/i),
      ).toBeInTheDocument();
    });
  });
});