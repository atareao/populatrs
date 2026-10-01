import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router";
import { ConfigProvider } from "antd";
import LoginPage from "../pages/LoginPage";

// --- JWT helpers -----------------------------------------------------------
function base64url(input: string): string {
  return btoa(input).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function makeJwt(payload: Record<string, unknown>): string {
  const header = base64url(JSON.stringify({ alg: "none", typ: "JWT" }));
  const body = base64url(JSON.stringify(payload));
  return `${header}.${body}.`;
}

const nowSeconds = () => Math.floor(Date.now() / 1000);

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

// Renders LoginPage with a real "/" route so navigation can be asserted.
function renderLoginWithRoutes() {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={["/login"]}>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route path="/" element={<div>DASHBOARD</div>} />
        </Routes>
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

  it("does not redirect and clears an expired token", async () => {
    sessionStorage.setItem(
      "populatrs_token",
      makeJwt({ exp: nowSeconds() - 3600 }),
    );
    renderLoginWithRoutes();

    await waitFor(() => {
      expect(sessionStorage.getItem("populatrs_token")).toBeNull();
    });
    expect(screen.queryByText("DASHBOARD")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /iniciar con oidc/i }),
    ).toBeInTheDocument();
  });

  it("redirects to the dashboard when a valid token is stored", async () => {
    sessionStorage.setItem(
      "populatrs_token",
      makeJwt({ exp: nowSeconds() + 3600 }),
    );
    renderLoginWithRoutes();

    await waitFor(() => {
      expect(screen.getByText("DASHBOARD")).toBeInTheDocument();
    });
  });

  it("shows the login button when no token is stored", () => {
    renderLoginWithRoutes();
    expect(
      screen.getByRole("button", { name: /iniciar con oidc/i }),
    ).toBeInTheDocument();
    expect(screen.queryByText("DASHBOARD")).not.toBeInTheDocument();
  });
});