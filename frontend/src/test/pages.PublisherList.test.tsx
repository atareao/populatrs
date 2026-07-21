import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ConfigProvider } from "antd";
import PublisherList from "../pages/Publishers/PublisherList";

const mockPublishersData = {
  publishers: {
    telegram: { type: "Telegram", config: { bot_token: "xxx", chat_id: "-123" }, enabled: true },
    x: { type: "X", config: { client_id: "cid", client_secret: "csec", redirect_uri: "http://localhost:5173/oauth/callback" }, enabled: true },
    mastodon: { type: "Mastodon", config: { server_url: "https://mastodon.social", access_token: "" }, enabled: false },
  },
  total: 3,
};

function mockMultiFetch(responses: Record<string, { status: number; body: unknown }>) {
  globalThis.fetch = vi.fn().mockImplementation((url: string) => {
    const match = responses[url];
    if (match) {
      return Promise.resolve({
        ok: match.status >= 200 && match.status < 300,
        status: match.status,
        json: () => Promise.resolve(match.body),
        text: () => Promise.resolve(JSON.stringify(match.body)),
      });
    }
    return Promise.resolve({
      ok: false,
      status: 404,
      json: () => Promise.resolve({ error: `No mock for ${url}` }),
      text: () => Promise.resolve(`No mock for ${url}`),
    });
  });
}

function renderPublisherList(initialEntries = ["/publishers"]) {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={initialEntries}>
        <PublisherList />
      </MemoryRouter>
    </ConfigProvider>,
  );
}

describe("PublisherList", () => {
  beforeEach(() => {
    sessionStorage.clear();
    vi.restoreAllMocks();
    sessionStorage.setItem("populatrs_token", "test-token");
  });

  it("renders the page title", async () => {
    mockMultiFetch({
      "/api/publishers": { status: 200, body: mockPublishersData },
    });
    renderPublisherList();

    await waitFor(() => {
      expect(screen.getByText("Publishers")).toBeInTheDocument();
    });
  });

  it("displays publishers in the table", async () => {
    mockMultiFetch({
      "/api/publishers": { status: 200, body: mockPublishersData },
    });
    renderPublisherList();

    await waitFor(() => {
      expect(screen.getByText("telegram")).toBeInTheDocument();
    });

    expect(screen.getByText("x")).toBeInTheDocument();
    expect(screen.getByText("mastodon")).toBeInTheDocument();
  });

  it("shows type tags for each publisher", async () => {
    mockMultiFetch({
      "/api/publishers": { status: 200, body: mockPublishersData },
    });
    renderPublisherList();

    await waitFor(() => {
      expect(screen.getByText("Telegram")).toBeInTheDocument();
    });

    expect(screen.getByText("X")).toBeInTheDocument();
    expect(screen.getByText("Mastodon")).toBeInTheDocument();
  });

  it("shows enabled toggle for publishers", async () => {
    mockMultiFetch({
      "/api/publishers": { status: 200, body: mockPublishersData },
    });
    renderPublisherList();

    await waitFor(() => {
      expect(screen.getByText("telegram")).toBeInTheDocument();
    });

    // All switches should be rendered
    const switches = screen.getAllByRole("switch");
    expect(switches.length).toBe(3);
  });

  it("shows enabled publisher with switch checked", async () => {
    mockMultiFetch({
      "/api/publishers": { status: 200, body: mockPublishersData },
    });
    renderPublisherList();

    await waitFor(() => {
      const switches = screen.getAllByRole("switch");
      // telegram is enabled
      expect(switches[0]).toBeChecked();
    });
  });

  it("shows Connect button for OAuth publishers without tokens", async () => {
    const noTokenPublishers = {
      publishers: {
        x: { type: "X", config: { client_id: "cid", client_secret: "csec", access_token: null, redirect_uri: "http://localhost:5173/oauth/callback" }, enabled: true },
      },
      total: 1,
    };
    mockMultiFetch({
      "/api/publishers": { status: 200, body: noTokenPublishers },
    });
    renderPublisherList();

    await waitFor(() => {
      expect(screen.getByText("X")).toBeInTheDocument();
    });

    const connectBtn = screen.getByRole("button", { name: /connect/i });
    expect(connectBtn).toBeInTheDocument();
  });

  it("opens edit modal on edit click", async () => {
    mockMultiFetch({
      "/api/publishers": { status: 200, body: mockPublishersData },
    });
    renderPublisherList();

    await waitFor(() => {
      expect(screen.getByText("telegram")).toBeInTheDocument();
    });

    const editButtons = screen.getAllByRole("button", { name: /edit/i });
    await userEvent.click(editButtons[0]);

    await waitFor(() => {
      expect(screen.getByText(/edit publisher/i)).toBeInTheDocument();
    });
  });

  it("submits edit form", async () => {
    mockMultiFetch({
      "/api/publishers": { status: 200, body: mockPublishersData },
    });
    renderPublisherList();

    await waitFor(() => {
      expect(screen.getByText("telegram")).toBeInTheDocument();
    });

    const editButtons = screen.getAllByRole("button", { name: /edit/i });
    await userEvent.click(editButtons[0]);

    await waitFor(() => {
      expect(screen.getByText(/edit publisher/i)).toBeInTheDocument();
    });

    // Mock PUT and subsequent reload
    mockMultiFetch({
      "/api/publishers": { status: 200, body: mockPublishersData },
      "/api/publishers/telegram": { status: 200, body: { status: "ok" } },
    });

    const okBtn = screen.getByRole("button", { name: /ok/i });
    await userEvent.click(okBtn);

    // After save, the list should reload
    await waitFor(() => {
      expect(screen.getByText("telegram")).toBeInTheDocument();
    });
  });
});