import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { ConfigProvider } from "antd";
import AppLayout from "../components/AppLayout";

// Mock useAuth hook
vi.mock("../hooks/useAuth", () => ({
  useAuth: () => ({ user: { sub: "test", email: "test@test.com", name: "Test" }, loading: false }),
}));

// Mock Outlet from react-router
vi.mock("react-router", async () => {
  const actual = await vi.importActual("react-router");
  return {
    ...actual,
    Outlet: () => <div data-testid="outlet">Outlet Content</div>,
  };
});

function renderLayout(initialEntries = ["/"]) {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={initialEntries}>
        <AppLayout />
      </MemoryRouter>
    </ConfigProvider>,
  );
}

describe("AppLayout", () => {
  beforeEach(() => {
    sessionStorage.clear();
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("renders the logo text", () => {
    renderLayout();
    expect(screen.getByText("populatrs")).toBeInTheDocument();
  });

  it("renders all navigation menu items", () => {
    renderLayout();
    expect(screen.getByText("Dashboard")).toBeInTheDocument();
    expect(screen.getByText("Feeds")).toBeInTheDocument();
    expect(screen.getByText("Publishers")).toBeInTheDocument();
    expect(screen.getByText("Logs")).toBeInTheDocument();
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("renders logout button", () => {
    renderLayout();
    expect(screen.getByText("Cerrar sesión")).toBeInTheDocument();
  });

  it("renders the collapse button with an accessible name", () => {
    renderLayout();
    expect(screen.getByRole("button", { name: "Contraer menú" })).toBeInTheDocument();
  });

  it("names the logout button when the sidebar is collapsed", () => {
    renderLayout();
    fireEvent.click(screen.getByRole("button", { name: "Contraer menú" }));
    expect(screen.getByRole("button", { name: "Cerrar sesión" })).toBeInTheDocument();
  });

  it("renders the Outlet content", () => {
    renderLayout();
    expect(screen.getByTestId("outlet")).toBeInTheDocument();
  });

  it("highlights the active menu item based on path", () => {
    renderLayout(["/feeds"]);
    const menuItems = screen.getAllByText("Feeds");
    expect(menuItems.length).toBeGreaterThan(0);
  });

  describe("logout", () => {
    const originalLocation = window.location;

    afterEach(() => {
      Object.defineProperty(window, "location", {
        writable: true,
        configurable: true,
        value: originalLocation,
      });
    });

    function mockLogoutFetch(endSessionUrl: string | null) {
      const fetchMock = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: () => Promise.resolve({ end_session_url: endSessionUrl }),
        text: () => Promise.resolve(""),
      });
      globalThis.fetch = fetchMock;
      return fetchMock;
    }

    function stubLocation() {
      Object.defineProperty(window, "location", {
        writable: true,
        configurable: true,
        value: { href: "" },
      });
    }

    it("calls POST /auth/logout, clears tokens, and navigates to /login", async () => {
      sessionStorage.setItem("populatrs_token", "raw-token");
      localStorage.setItem("populatrs_token", "raw-token");
      const fetchMock = mockLogoutFetch(null);
      stubLocation();

      renderLayout();
      fireEvent.click(screen.getByText("Cerrar sesión"));

      await waitFor(() => expect(window.location.href).toBe("/login"));
      expect(fetchMock).toHaveBeenCalledWith(
        "/auth/logout",
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({
            Authorization: "Bearer raw-token",
          }),
        }),
      );
      expect(sessionStorage.getItem("populatrs_token")).toBeNull();
      expect(localStorage.getItem("populatrs_token")).toBeNull();
    });

    it("navigates to the provider end_session_url when present", async () => {
      sessionStorage.setItem("populatrs_token", "raw-token");
      mockLogoutFetch("https://idp.example.com/logout");
      stubLocation();

      renderLayout();
      fireEvent.click(screen.getByText("Cerrar sesión"));

      await waitFor(() =>
        expect(window.location.href).toBe("https://idp.example.com/logout"),
      );
    });

    it("falls back to /login for a javascript: end_session_url", async () => {
      sessionStorage.setItem("populatrs_token", "raw-token");
      mockLogoutFetch("javascript:alert(1)");
      stubLocation();

      renderLayout();
      fireEvent.click(screen.getByText("Cerrar sesión"));

      await waitFor(() => expect(window.location.href).toBe("/login"));
    });

    it("falls back to /login for an empty end_session_url", async () => {
      sessionStorage.setItem("populatrs_token", "raw-token");
      mockLogoutFetch("");
      stubLocation();

      renderLayout();
      fireEvent.click(screen.getByText("Cerrar sesión"));

      await waitFor(() => expect(window.location.href).toBe("/login"));
    });

    it("does not call logout twice on a double-click", async () => {
      sessionStorage.setItem("populatrs_token", "raw-token");
      let resolveFetch: ((value: unknown) => void) | undefined;
      const fetchMock = vi.fn().mockImplementation(
        () =>
          new Promise((resolve) => {
            resolveFetch = resolve;
          }),
      );
      globalThis.fetch = fetchMock;
      stubLocation();

      renderLayout();
      const btn = screen.getByRole("button", { name: "Cerrar sesión" });
      fireEvent.click(btn);
      fireEvent.click(btn);

      expect(fetchMock).toHaveBeenCalledTimes(1);

      resolveFetch?.({
        ok: true,
        status: 200,
        json: () => Promise.resolve({ end_session_url: null }),
        text: () => Promise.resolve(""),
      });
      await waitFor(() => expect(window.location.href).toBe("/login"));
    });

    it("re-enables the logout button after logout completes", async () => {
      sessionStorage.setItem("populatrs_token", "raw-token");
      mockLogoutFetch(null);
      stubLocation();

      renderLayout();
      const btn = screen.getByRole("button", { name: "Cerrar sesión" });
      fireEvent.click(btn);

      await waitFor(() => expect(window.location.href).toBe("/login"));
      await waitFor(() => expect(btn).not.toBeDisabled());
    });
  });
});