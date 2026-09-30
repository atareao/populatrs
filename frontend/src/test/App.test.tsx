import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { ConfigProvider } from "antd";
import App from "../App";
import { setToken } from "../store/auth";

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

function renderApp(initialEntries = ["/"]) {
  return render(
    <ConfigProvider>
      <MemoryRouter initialEntries={initialEntries}>
        <App />
      </MemoryRouter>
    </ConfigProvider>,
  );
}

// URL-aware fetch mock so the lazy Dashboard does not crash after recovery.
function mockBackendReachable() {
  globalThis.fetch = vi.fn().mockImplementation((url: string) => {
    const body =
      url === "/api/me"
        ? { sub: "u", email: "recovered@test.com", name: "Recovered" }
        : {
            feeds: { total: 0, enabled: 0, disabled: 0 },
            publishers: { total: 0 },
            published_posts: 0,
            schedule: { cron_expression: "", timezone: "" },
            storage: { data_dir: "" },
            last_run_at: null,
            next_run_at: null,
          };
    return Promise.resolve({
      ok: true,
      status: 200,
      json: () => Promise.resolve(body),
      text: () => Promise.resolve(""),
    });
  });
}

describe("App ProtectedRoute", () => {
  beforeEach(() => {
    sessionStorage.clear();
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("shows a retry screen (not /login) on a network error with a locally-valid token", async () => {
    setToken(makeJwt({ exp: nowSeconds() + 3600 }));
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("Network failure"));

    renderApp(["/"]);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /reintentar/i }),
      ).toBeInTheDocument();
    });
    // Must NOT bounce to the login page.
    expect(
      screen.queryByRole("button", { name: /iniciar con oidc/i }),
    ).not.toBeInTheDocument();
  });

  it("recovers and shows the protected area after retry succeeds", async () => {
    setToken(makeJwt({ exp: nowSeconds() + 3600 }));
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("Network failure"));

    renderApp(["/"]);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /reintentar/i }),
      ).toBeInTheDocument(),
    );

    mockBackendReachable();
    fireEvent.click(screen.getByRole("button", { name: /reintentar/i }));

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Cerrar sesión" }),
      ).toBeInTheDocument();
    });
    expect(
      screen.queryByRole("button", { name: /reintentar/i }),
    ).not.toBeInTheDocument();
  });

  it("offers an escape hatch to /login from the retry screen", async () => {
    setToken(makeJwt({ exp: nowSeconds() + 3600 }));
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("Network failure"));

    renderApp(["/"]);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /reintentar/i }),
      ).toBeInTheDocument(),
    );

    fireEvent.click(
      screen.getByRole("button", { name: /ir a iniciar sesión/i }),
    );

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /iniciar con oidc/i }),
      ).toBeInTheDocument();
    });
    expect(sessionStorage.getItem("populatrs_token")).toBeNull();
  });
});
