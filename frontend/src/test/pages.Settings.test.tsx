import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ConfigProvider } from "antd";
import Settings from "../pages/Settings";

const mockYoutube = { api_key: "AIzaSyTest123" };
const emptyYoutube = { api_key: "" };

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
  });

  it("shows loading spinner initially", () => {
    globalThis.fetch = vi.fn().mockReturnValue(new Promise(() => {}));
    renderSettings();
    expect(screen.getByTestId("spinner")).toBeInTheDocument();
  });

  it("renders the settings page title", async () => {
    mockFetch(200, mockYoutube);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Settings")).toBeInTheDocument();
    });
  });

  it("loads and displays youtube config section", async () => {
    mockFetch(200, mockYoutube);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("YouTube Configuration")).toBeInTheDocument();
    });
  });

  it("saves youtube config on form submit", async () => {
    mockFetch(200, mockYoutube);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("YouTube Configuration")).toBeInTheDocument();
    });

    // Mock PUT response
    mockFetch(200, { status: "ok" });

    const saveBtn = screen.getByRole("button", { name: /save/i });
    await userEvent.click(saveBtn);

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/youtube",
        expect.objectContaining({ method: "PUT" }),
      );
    });
  });

  it("shows info about youtube api key", async () => {
    mockFetch(200, emptyYoutube);
    renderSettings();

    await waitFor(() => {
      expect(
        screen.getByText(/YouTube Data API v3 key/i),
      ).toBeInTheDocument();
    });
  });
});