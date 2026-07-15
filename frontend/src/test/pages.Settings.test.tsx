import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ConfigProvider } from "antd";
import Settings from "../pages/Settings";

const mockStorage = {
  data_dir: "/app/data",
  published_posts_file: "posts.json",
};

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
    mockFetch(200, mockStorage);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Settings")).toBeInTheDocument();
    });
  });

  it("loads and displays current configuration", async () => {
    mockFetch(200, mockStorage);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Current Configuration")).toBeInTheDocument();
    });

    expect(screen.getByText("/app/data")).toBeInTheDocument();
    expect(screen.getByText("posts.json")).toBeInTheDocument();
  });

  it("renders edit form with loaded values", async () => {
    mockFetch(200, mockStorage);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Edit Storage Config")).toBeInTheDocument();
    });

    const inputs = screen.getAllByRole("textbox");
    expect(inputs.length).toBeGreaterThanOrEqual(2);
  });

  it("saves storage config on form submit", async () => {
    mockFetch(200, mockStorage);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Edit Storage Config")).toBeInTheDocument();
    });

    // Mock the PUT response
    mockFetch(200, { status: "ok" });

    const saveBtn = screen.getByRole("button", { name: /save changes/i });
    await userEvent.click(saveBtn);

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/storage",
        expect.objectContaining({ method: "PUT" }),
      );
    });
  });

  it("shows warning about restart requirement", async () => {
    mockFetch(200, mockStorage);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText(/will apply after a server restart/i)).toBeInTheDocument();
    });
  });
});