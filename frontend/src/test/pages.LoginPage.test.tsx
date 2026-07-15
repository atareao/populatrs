import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
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

  it("renders login button", () => {
    renderLogin();
    const loginBtn = screen.getByRole("button", { name: /iniciar sesión/i });
    expect(loginBtn).toBeInTheDocument();
  });

  it("renders dev login input and button", () => {
    renderLogin();
    expect(screen.getByPlaceholderText("Email para dev login")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /dev/i })).toBeInTheDocument();
  });

  it("login button redirects to /auth/login", () => {
    renderLogin();
    const loginBtn = screen.getByRole("button", { name: /iniciar sesión/i });
    fireEvent.click(loginBtn);
    expect(window.location.href).toBe("/auth/login");
  });

  it("dev login redirects with email", () => {
    renderLogin();
    const input = screen.getByPlaceholderText("Email para dev login");
    fireEvent.change(input, { target: { value: "dev@example.com" } });

    const devBtn = screen.getByRole("button", { name: /dev/i });
    fireEvent.click(devBtn);

    expect(window.location.href).toContain("/auth/dev-login");
    expect(window.location.href).toContain("dev%40example.com");
  });

  it("renders without crashing when sessionStorage has a token", () => {
    sessionStorage.setItem("populatrs_token", "existing-token");
    renderLogin();
    // Should render without error - the token redirect is handled by useEffect
    expect(screen.getByText("populatrs")).toBeInTheDocument();
  });
});