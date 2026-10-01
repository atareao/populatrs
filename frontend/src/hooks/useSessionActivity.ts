import { useEffect, useRef } from "react";
import { useNavigate } from "react-router";
import { sendActivity, SessionExpiredError } from "../api/http";
import {
  clearToken,
  ensureSessionWindow,
  getToken,
  isSessionWindowValid,
  recordActivity,
} from "../store/auth";

/** At most one activity signal per this window (mirrors the server default). */
export const ACTIVITY_THROTTLE_MS = 60_000;

/** How often the client checks its locally-tracked session window. */
export const SESSION_CHECK_INTERVAL_MS = 30_000;

/** Genuine user-interaction events. Background polling never dispatches these. */
const ACTIVITY_EVENTS = [
  "pointerdown",
  "mousedown",
  "keydown",
  "touchstart",
  "wheel",
] as const;

/**
 * Best-effort client session tracker (D6).
 *
 * - Sends an explicit activity signal on genuine user interaction, throttled.
 * - Ignores interactions while the tab is hidden.
 * - Periodically ends the local session and returns to `/login` when the idle
 *   window or the absolute lifetime has passed.
 *
 * The server stays authoritative: a server 401 still clears the session through
 * the shared HTTP interceptor.
 */
export function useSessionActivity(): void {
  const navigate = useNavigate();
  const lastSentRef = useRef(0);

  useEffect(() => {
    // Anchor the window for server-injected logins (OIDC callback / dev-login).
    if (getToken()) {
      ensureSessionWindow();
    }

    const handleActivity = (): void => {
      if (!getToken()) return;
      // Single source of truth for tab visibility.
      if (document.visibilityState !== "visible") return;

      const now = Date.now();
      if (now - lastSentRef.current < ACTIVITY_THROTTLE_MS) return;
      lastSentRef.current = now;

      // Extend the local window immediately (best-effort) and tell the server.
      recordActivity(now);
      sendActivity().catch((e: unknown) => {
        // The 401 interceptor already cleared the token; send the user to login.
        if (e instanceof SessionExpiredError) {
          navigate("/login", { replace: true });
        }
      });
    };

    ACTIVITY_EVENTS.forEach((event) =>
      window.addEventListener(event, handleActivity, { passive: true }),
    );

    return () => {
      ACTIVITY_EVENTS.forEach((event) =>
        window.removeEventListener(event, handleActivity),
      );
    };
  }, [navigate]);

  useEffect(() => {
    const check = (): void => {
      if (!getToken()) return;
      if (!isSessionWindowValid()) {
        clearToken();
        navigate("/login", { replace: true });
      }
    };

    // Check once on mount, then periodically while the app is open.
    check();
    const intervalId = window.setInterval(check, SESSION_CHECK_INTERVAL_MS);
    return () => window.clearInterval(intervalId);
  }, [navigate]);
}
