import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { ConfigProvider } from "antd";
import Dashboard from "../pages/Dashboard";

const mockStatus = {
  feeds: { total: 10, enabled: 7, disabled: 3 },
  publishers: { total: 4 },
  published_posts: 128,
  schedule: { cron_expression: "0 * * * *", timezone: "Europe/Madrid" },
  storage: { data_dir: "./data" },
  last_run_at: "2026-07-25T20:00:16+00:00",
  next_run_at: "2026-07-25T21:00:00+00:00",
};

function mockFetch(status: number, body: unknown) {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
    text: () => Promise.resolve(typeof body === "string" ? body : JSON.stringify(body)),
  });
}

function renderDashboard(initialEntries = ["/"]) {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={initialEntries}>
        <Dashboard />
      </MemoryRouter>
    </ConfigProvider>,
  );
}

describe("Dashboard", () => {
  beforeEach(() => {
    sessionStorage.clear();
    vi.restoreAllMocks();
    sessionStorage.setItem("populatrs_token", "test-token");
  });

  it("shows loading spinner initially", () => {
    mockFetch(200, mockStatus);
    // Don't resolve the fetch promise immediately
    globalThis.fetch = vi.fn().mockReturnValue(new Promise(() => {}));
    renderDashboard();
    expect(screen.getByTestId("spinner")).toBeInTheDocument();
  });

  it("renders feed stats from API", async () => {
    mockFetch(200, mockStatus);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Feeds Activos")).toBeInTheDocument();
    });

    expect(screen.getByText("7")).toBeInTheDocument();
    expect(screen.getByText("/ 10")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("renders publishers count", async () => {
    mockFetch(200, mockStatus);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("4")).toBeInTheDocument();
    });
  });

  it("renders published posts count", async () => {
    mockFetch(200, mockStatus);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("128")).toBeInTheDocument();
    });
  });

  it("renders schedule info", async () => {
    mockFetch(200, mockStatus);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Schedule")).toBeInTheDocument();
    });

    expect(screen.getByText("0 * * * *")).toBeInTheDocument();
    expect(screen.getByText("Europe/Madrid")).toBeInTheDocument();
  });

  it("renders storage info", async () => {
    mockFetch(200, mockStatus);
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Storage")).toBeInTheDocument();
    });

    expect(screen.getByText("./data")).toBeInTheDocument();
  });

  it("shows error when API fails", async () => {
    mockFetch(500, { error: "Server error" });
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Failed to load dashboard data")).toBeInTheDocument();
    });
  });

  it("shows all zeroes when API returns empty data", async () => {
    mockFetch(200, {
      feeds: { total: 0, enabled: 0, disabled: 0 },
      publishers: { total: 0 },
      published_posts: 0,
      schedule: { cron_expression: "0 * * * *", timezone: "UTC" },
      storage: { data_dir: "./data" },
      last_run_at: null,
      next_run_at: null,
    });
    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Feeds Activos")).toBeInTheDocument();
    });

    const zeros = screen.getAllByText("0");
    expect(zeros.length).toBeGreaterThanOrEqual(3);
  });
});
