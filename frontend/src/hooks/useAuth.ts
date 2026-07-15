import { useState, useEffect } from "react";
import { fetchMe, type User } from "../api/http";
import { clearToken } from "../store/auth";

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
        clearToken();
        setUser(null);
      })
      .finally(() => setLoading(false));
  }, []);

  return { user, loading };
}

export { getToken };