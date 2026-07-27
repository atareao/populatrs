import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { ConfigProvider } from "antd";
import LoginPage from "../pages/LoginPage";

const originalLocation = window.location;

beforeEach(() => {
  sessionStorage.clear();
  localStorage.clear();
  vi.restoreAllMocks();

  Object.defineProperty(window, "location", {
    writable: true,
    value: { ...originalLocation, href: "" },
  });
});

afterEach(() => {
  Object.defineProperty(window, "location", {
    writable: true,
    value: originalLocation,
  });
});

function renderLogin(initialEntries = ["/login"]) {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={initialEntries}>
        <LoginPage />
      </MemoryRouter>
    </ConfigProvider>,
  );
}

describe("LoginPage", () => {
  it("renders the logo and title", () => {
    renderLogin();
    expect(screen.getByText("populatrs")).toBeInTheDocument();
    expect(screen.getByText("Automatic RSS feed publisher")).toBeInTheDocument();
  });

  it("renders OIDC login button", () => {
    renderLogin();
    const loginBtn = screen.getByRole("button", { name: /iniciar con oidc/i });
    expect(loginBtn).toBeInTheDocument();
  });

  it("login button redirects to /auth/login", () => {
    renderLogin();
    const loginBtn = screen.getByRole("button", { name: /iniciar con oidc/i });
    fireEvent.click(loginBtn);
    expect(window.location.href).toBe("/auth/login");
  });

  it("renders without crashing when sessionStorage has a token", () => {
    sessionStorage.setItem("populatrs_token", "existing-token");
    renderLogin();
    // Should render without error - the token redirect is handled by useEffect
    expect(screen.getByText("populatrs")).toBeInTheDocument();
  });
});