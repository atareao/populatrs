import { useState, useEffect, useCallback } from "react";
import { fetchMe, SessionExpiredError, type User } from "../api/http";
import { getToken, setSessionLimits } from "../store/auth";

export function useAuth() {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  const check = useCallback(() => {
    const token = getToken();
    if (!token) {
      setUser(null);
      setError(false);
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(false);

    fetchMe()
      .then((u) => {
        // Adopt the server-configured session limits when provided; a missing
        // value leaves the current/default limits untouched.
        setSessionLimits(u.idle_seconds, u.absolute_seconds);
        setUser(u);
        setError(false);
      })
      .catch((e: unknown) => {
        // NO limpiar token aquí — el interceptor de 401 ya lo hizo
        // si el refresh falló. Si el error es de red, el token sigue
        // siendo válido y no debemos borrarlo.
        setUser(null);
        // Only a genuine session expiry should send the user to /login.
        // Transient failures (network/server) surface a retryable error.
        setError(!(e instanceof SessionExpiredError));
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    check();
  }, [check]);

  return { user, loading, error, retry: check };
}
