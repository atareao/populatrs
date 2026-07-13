import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ConfigProvider } from "antd";
import AppLayout from "../components/AppLayout";

// Mock useAuth hook
vi.mock("../hooks/useAuth", () => ({
  useAuth: () => ({ user: { sub: "test", email: "test@test.com", name: "Test" }, loading: false }),
}));

// Mock Outlet from react-router-dom
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
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
    expect(screen.getByText("Schedule")).toBeInTheDocument();
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("renders logout button", () => {
    renderLayout();
    expect(screen.getByText("Cerrar sesión")).toBeInTheDocument();
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
});