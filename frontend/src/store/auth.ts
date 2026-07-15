export function getToken(): string | null {
  try {
    return sessionStorage.getItem("populatrs_token") || localStorage.getItem("populatrs_token");
  } catch {
    return null;
  }
}

export function setToken(token: string): void {
  try {
    sessionStorage.setItem("populatrs_token", token);
    localStorage.setItem("populatrs_token", token);
  } catch { /* noop */ }
}

export function clearToken(): void {
  try {
    sessionStorage.removeItem("populatrs_token");
    localStorage.removeItem("populatrs_token");
  } catch { /* noop */ }
}