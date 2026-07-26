import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ConfigProvider } from "antd";
import Schedule from "../pages/Schedule";

// Mock antd message to prevent errors in test environment
vi.mock("antd", async () => {
  const actual = await vi.importActual("antd");
  return {
    ...actual,
    message: {
      success: vi.fn(),
      error: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
      loading: vi.fn(),
      destroy: vi.fn(),
    },
  };
});

const mockSchedule = {
  cron_expression: "*/30 * * * *",
  timezone: "Europe/Madrid",
};

function mockFetch(status: number, body: unknown) {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
    text: () => Promise.resolve(typeof body === "string" ? body : JSON.stringify(body)),
  });
}

function renderSchedule(initialEntries = ["/schedule"]) {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={initialEntries}>
        <Schedule />
      </MemoryRouter>
    </ConfigProvider>,
  );
}

describe("Schedule", () => {
  beforeEach(() => {
    sessionStorage.clear();
    vi.restoreAllMocks();
    sessionStorage.setItem("populatrs_token", "test-token");
  });

  it("shows loading spinner initially", () => {
    globalThis.fetch = vi.fn().mockReturnValue(new Promise(() => {}));
    renderSchedule();
    expect(screen.getByTestId("spinner")).toBeInTheDocument();
  });

  it("renders the schedule page title", async () => {
    mockFetch(200, mockSchedule);
    renderSchedule();

    await waitFor(() => {
      expect(screen.getByText("Schedule")).toBeInTheDocument();
    });
  });

  it("loads and displays current configuration", async () => {
    mockFetch(200, mockSchedule);
    renderSchedule();

    await waitFor(() => {
      expect(screen.getByText("Current Configuration")).toBeInTheDocument();
    });

    expect(screen.getByText("*/30 * * * *")).toBeInTheDocument();
    expect(screen.getByText("Europe/Madrid")).toBeInTheDocument();
  });

  it("renders edit form with loaded values", async () => {
    mockFetch(200, mockSchedule);
    renderSchedule();

    await waitFor(() => {
      expect(screen.getByText("Edit Schedule")).toBeInTheDocument();
    });

    const cronInput = screen.getByRole("textbox", { name: /cron/i });
    expect(cronInput).toHaveValue("*/30 * * * *");
  });

  it("saves updated schedule on form submit", async () => {
    mockFetch(200, mockSchedule);
    renderSchedule();

    await waitFor(() => {
      expect(screen.getByText("Edit Schedule")).toBeInTheDocument();
    });

    // Mock the PUT response
    mockFetch(200, { status: "ok" });

    const cronInput = screen.getByRole("textbox", { name: /cron/i });
    await userEvent.clear(cronInput);
    await userEvent.type(cronInput, "0 */2 * * *");

    const saveBtn = screen.getByRole("button", { name: /save changes/i });
    await userEvent.click(saveBtn);

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/schedule",
        expect.objectContaining({ method: "PUT" }),
      );
    });
  });

  it("renders form with defaults on load failure", async () => {
    mockFetch(500, "Server error");
    renderSchedule();

    await waitFor(() => {
      expect(screen.getByText("Edit Schedule")).toBeInTheDocument();
    }, { timeout: 3000 });
  });
});