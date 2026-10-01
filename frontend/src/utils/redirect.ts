/**
 * Validates a logout redirect target.
 *
 * Returns the normalized URL (`parsed.href`) only when it is an absolute
 * `http`/`https` URL. Anything else — a non-http(s) scheme such as
 * `javascript:`, a relative path, an empty string, or `null` — falls back to
 * `/login`.
 */
export function safeRedirectTarget(url: string | null): string {
  if (!url) return "/login";

  try {
    const parsed = new URL(url);
    if (parsed.protocol === "http:" || parsed.protocol === "https:") {
      return parsed.href;
    }
  } catch {
    // Not an absolute URL.
  }

  return "/login";
}
