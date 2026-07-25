import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { ConfigProvider } from "antd";
import FeedList from "../pages/Feeds/FeedList";

const mockFeeds = {
  feeds: [
    { id: "blog", name: "My Blog", type: "Rss", enabled: true, publishers: ["telegram"], config: { url: "https://blog.com/feed.xml" }, check_interval_minutes: 60 },
    { id: "youtube", name: "My Channel", type: "Youtube", enabled: false, publishers: [], config: { channel_id: "UC123" }, check_interval_minutes: null },
  ],
  total: 2,
};

const mockPublishers = {
  publishers: { telegram: { type: "Telegram", config: { bot_token: "xxx" }, enabled: true } },
  total: 1,
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
    return Promise.reject(new Error(`No mock for ${url}`));
  });
}

function renderFeedList(initialEntries = ["/feeds"]) {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={initialEntries}>
        <FeedList />
      </MemoryRouter>
    </ConfigProvider>,
  );
}

describe("FeedList", () => {
  beforeEach(() => {
    sessionStorage.clear();
    vi.restoreAllMocks();
    sessionStorage.setItem("populatrs_token", "test-token");
  });

  it("renders the page title", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("Feeds")).toBeInTheDocument();
    });
  });

  it("displays feeds in the table", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("My Blog")).toBeInTheDocument();
    });

    expect(screen.getByText("My Channel")).toBeInTheDocument();
    expect(screen.getByText("Rss")).toBeInTheDocument();
    expect(screen.getByText("Youtube")).toBeInTheDocument();
  });

  it("shows add feed button", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("Add Feed")).toBeInTheDocument();
    });
  });

  it("opens create modal on add feed click", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("Add Feed")).toBeInTheDocument();
    });

    const addBtn = screen.getByRole("button", { name: /add feed/i });
    await userEvent.click(addBtn);

    await waitFor(() => {
      expect(screen.getByText("Create Feed")).toBeInTheDocument();
    });
  });

  it("opens edit modal on edit click", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("My Blog")).toBeInTheDocument();
    });

    const editButtons = screen.getAllByRole("button", { name: /edit/i });
    await userEvent.click(editButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Edit Feed")).toBeInTheDocument();
    });
  });

  it("shows enabled/disabled status with emoji", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("My Blog")).toBeInTheDocument();
    });

    // My Blog is enabled (✅), My Channel is disabled (❌)
    expect(screen.getByText("✅")).toBeInTheDocument();
    expect(screen.getByText("❌")).toBeInTheDocument();
  });

  it("calls run with publish=true when execute is clicked", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("My Blog")).toBeInTheDocument();
    });

    // Click Run → confirmation modal opens
    const runButtons = screen.getAllByRole("button", { name: /run/i });
    await userEvent.click(runButtons[0]);

    // Wait for the run modal with the switch
    await waitFor(() => {
      expect(screen.getByText("Publish to publishers")).toBeInTheDocument();
    });

    // Switch is ON by default (publish=true)
    // Click Execute
    const executeButton = screen.getByRole("button", { name: /execute/i });
    await userEvent.click(executeButton);

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledWith(
        "/api/feeds/blog/run?publish=true",
        expect.objectContaining({ method: "POST" }),
      );
    });
  });

  it("shows delete confirmation on delete click", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("My Blog")).toBeInTheDocument();
    });

    const deleteButtons = screen.getAllByRole("button", { name: /delete/i });
    await userEvent.click(deleteButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("Delete this feed?")).toBeInTheDocument();
    });
  });

  it("submits create feed form", async () => {
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });
    renderFeedList();

    await waitFor(() => {
      expect(screen.getByText("Add Feed")).toBeInTheDocument();
    });

    const addBtn = screen.getByRole("button", { name: /add feed/i });
    await userEvent.click(addBtn);

    await waitFor(() => {
      expect(screen.getByText("Create Feed")).toBeInTheDocument();
    });

    // Mock the POST response
    mockMultiFetch({
      "/api/feeds": { status: 200, body: mockFeeds },
      "/api/publishers": { status: 200, body: mockPublishers },
    });

    const okBtn = screen.getByRole("button", { name: /ok/i });
    await userEvent.click(okBtn);
  });
});