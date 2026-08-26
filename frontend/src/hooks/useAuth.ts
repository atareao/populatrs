import { useState, useEffect } from "react";
import { fetchMe, type User } from "../api/http";

function getToken(): string | null {
  try {
    return sessionStorage.getItem("populatrs_token") || localStorage.getItem("populatrs_token");
  } catch {
    return null;
  }
}

export function useAuth() {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const token = getToken();
    if (!token) {
      setLoading(false);
      return;
    }

    fetchMe()
      .then(setUser)
      .catch(() => {
          // NO limpiar token aquí — el interceptor de 401 ya lo hizo
          // si el refresh falló. Si el error es de red, el token sigue
          // siendo válido y no debemos borrarlo.
          setUser(null);
        })
      .finally(() => setLoading(false));
  }, []);

  return { user, loading };
}

export { getToken };