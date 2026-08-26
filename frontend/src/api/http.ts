export interface User {
  sub: string;
  email: string;
  name: string;
}

export interface FeedPublisherBinding {
  publisher_id: string;
  template?: string | null;
}

export interface FeedConfig {
  id: string;
  type: string;
  config: Record<string, unknown>;
  name: string;
  enabled: boolean;
  publishers: FeedPublisherBinding[];
  max_retries?: number;
  retry_delay_seconds?: number;
}

export interface PublisherConfigEntry {
  type: string;
  config: Record<string, unknown> & { template?: string; reply_template?: string };
  enabled: boolean;
}

export interface ScheduleConfig {
  cron_expression: string;
  timezone: string;
  next_run_at: string | null;
}

export interface StorageConfig {
  data_dir: string;
}

export interface DashboardStatus {
  feeds: { total: number; enabled: number; disabled: number };
  publishers: { total: number };
  published_posts: number;
  schedule: { cron_expression: string; timezone: string };
  storage: { data_dir: string };
  last_run_at: string | null;
  next_run_at: string | null;
}

import { getToken, setToken, clearToken } from "../store/auth";

// Estado global para evitar múltiples refrescos concurrentes
let refreshPromise: Promise<string | null> | null = null;

async function refreshAccessToken(): Promise<string | null> {
  const currentToken = getToken();
  if (!currentToken) return null;

  try {
    const res = await fetch("/auth/refresh", {
      method: "POST",
      headers: { Authorization: `Bearer ${currentToken}` },
    });
    if (!res.ok) return null;
    const data = await res.json();
    const newToken = data.access_token;
    setToken(newToken);
    return newToken;
  } catch {
    return null;
  }
}

async function fetcher<T>(path: string, opts?: { method?: string; body?: unknown }): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(path, {
    method: opts?.method ?? (opts?.body ? "POST" : "GET"),
    headers,
    body: opts?.body ? JSON.stringify(opts.body) : undefined,
  });

  // 🔄 Interceptor 401: intentar refresh antes de fallar
  if (res.status === 401 && token) {
    // Evitar múltiples refrescos concurrentes
    if (!refreshPromise) {
      refreshPromise = refreshAccessToken().finally(() => {
        refreshPromise = null;
      });
    }

    const newToken = await refreshPromise;
    if (newToken) {
      // Reintentar con el nuevo token
      headers["Authorization"] = `Bearer ${newToken}`;
      const retry = await fetch(path, {
        method: opts?.method ?? (opts?.body ? "POST" : "GET"),
        headers,
        body: opts?.body ? JSON.stringify(opts.body) : undefined,
      });
      if (retry.ok) {
        if (retry.status === 204) return undefined as T;
        return retry.json();
      }
    }

    // Refresh falló → limpiar y dejar que ProtectedRoute redirija
    clearToken();
    throw new Error("Session expired");
  }

  if (!res.ok) {
    const text = await res.text().catch(() => "unknown error");
    throw new Error(`HTTP ${res.status}: ${text}`);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}

// Auth
export async function fetchMe(): Promise<User> {
  return fetcher<User>("/api/me");
}

// Feeds
export async function fetchFeeds(): Promise<{ feeds: FeedConfig[]; total: number }> {
  return fetcher("/api/feeds");
}

export async function createFeed(feed: FeedConfig): Promise<void> {
  return fetcher("/api/feeds", { method: "POST", body: feed });
}

export async function updateFeed(id: string, feed: FeedConfig): Promise<void> {
  return fetcher(`/api/feeds/${id}`, { method: "PUT", body: feed });
}

export async function deleteFeed(id: string): Promise<void> {
  return fetcher(`/api/feeds/${id}`, { method: "DELETE" });
}

export async function toggleFeed(id: string): Promise<{ enabled: boolean }> {
  return fetcher(`/api/feeds/${id}`, { method: "PATCH" });
}

// Publishers
export async function fetchPublishers(): Promise<{ publishers: Record<string, PublisherConfigEntry>; total: number }> {
  return fetcher("/api/publishers");
}

export async function createPublisher(id: string, config: PublisherConfigEntry): Promise<void> {
  return fetcher("/api/publishers", { method: "POST", body: { id, config } });
}

export async function updatePublisher(id: string, config: PublisherConfigEntry): Promise<void> {
  return fetcher(`/api/publishers/${id}`, { method: "PUT", body: { config } });
}

export async function testPublisher(id: string): Promise<{ status: string; message: string }> {
  return fetcher(`/api/publishers/${id}/test`, { method: "POST" });
}

export async function deletePublisher(id: string): Promise<void> {
  return fetcher(`/api/publishers/${id}`, { method: "DELETE" });
}

export async function togglePublisher(id: string): Promise<{ enabled: boolean }> {
  return fetcher(`/api/publishers/${id}`, { method: "PATCH" });
}

// Schedule
export async function fetchSchedule(): Promise<ScheduleConfig> {
  return fetcher("/api/schedule");
}

export async function updateSchedule(schedule: ScheduleConfig): Promise<void> {
  return fetcher("/api/schedule", { method: "PUT", body: schedule });
}

// Storage
export async function fetchStorage(): Promise<StorageConfig> {
  return fetcher("/api/storage");
}

export async function updateStorage(storage: StorageConfig): Promise<void> {
  return fetcher("/api/storage", { method: "PUT", body: storage });
}

// Run feed manually
// publish=true: fetch + publish to publishers + mark as published
// publish=false: fetch + mark as published only (no publisher results)
// dry_run=true: fetch + preview only, no side effects
export async function runFeed(id: string, publish = false): Promise<{ posts_count: number; posts: { guid: string; title: string; url: string }[] }> {
  return fetcher(`/api/feeds/${id}/run?publish=${publish}`, { method: "POST" });
}

export async function dryRunFeed(id: string): Promise<{ posts_count: number; posts: { guid: string; title: string; url: string }[]; dry_run: boolean; message: string }> {
  return fetcher(`/api/feeds/${id}/run?dry_run=true`, { method: "POST" });
}

export async function resolveYoutubeUrl(url: string): Promise<{ channel_id: string }> {
  return fetcher("/api/feeds/resolve-youtube", { method: "POST", body: { url } });
}

// OAuth
export interface OAuthStatus {
  ok: boolean;
  connected: boolean;
  token_expires_at: number | null;
  has_refresh_token: boolean;
  publisher_type: string;
}

export async function getOAuthUrl(id: string): Promise<{ url: string }> {
  return fetcher(`/api/publishers/${id}/oauth/authorize`);
}

export async function oauthCallback(id: string, code: string, state: string): Promise<{ success: boolean; message: string }> {
  return fetcher(`/api/publishers/${id}/oauth/callback`, {
    method: "POST",
    body: { code, state },
  });
}

export async function fetchOAuthStatus(id: string): Promise<OAuthStatus> {
  return fetcher<OAuthStatus>(`/api/publishers/${id}/oauth/status`);
}

// Logs
export interface FeedLogPublisherResult {
  publisher_id: string;
  success: boolean;
  message: string;
}

export interface FeedLogEntry {
  guid: string;
  feed_id: string;
  title: string;
  url: string;
  published_at: string;
  publisher_results: FeedLogPublisherResult[];
}

export interface FeedLogResponse {
  entries: FeedLogEntry[];
  total: number;
  retention_days: number;
}

export async function fetchFeedLogs(limit = 50, offset = 0): Promise<FeedLogResponse> {
  return fetcher(`/api/logs/history?limit=${limit}&offset=${offset}`);
}

export async function fetchLogRetention(): Promise<{ retention_days: number }> {
  return fetcher("/api/logs/retention");
}

export async function updateLogRetention(days: number): Promise<{ status: string; retention_days: number }> {
  return fetcher("/api/logs/retention", { method: "PUT", body: { retention_days: days } });
}

export async function republishPost(
  guid: string,
  feed_id: string,
  publisher_id: string,
): Promise<{ status: string; success: boolean; message: string }> {
  return fetcher("/api/logs/republish", { method: "POST", body: { guid, feed_id, publisher_id } });
}

// Status
export async function fetchStatus(): Promise<DashboardStatus> {
  return fetcher("/api/status");
}

// YouTube config
export interface YouTubeConfig {
  api_key: string;
}

export async function fetchYoutubeConfig(): Promise<YouTubeConfig> {
  return fetcher("/api/youtube");
}

export async function updateYoutubeConfig(config: YouTubeConfig): Promise<void> {
  return fetcher("/api/youtube", { method: "PUT", body: config });
}

// Retry policy
export interface RetryPolicy {
  max_retries: number;
  base_delay_seconds: number;
  max_delay_seconds: number;
  backoff_multiplier: number;
}

export async function fetchRetryPolicy(): Promise<RetryPolicy> {
  return fetcher<RetryPolicy>("/api/settings/retry-policy");
}

export async function updateRetryPolicy(policy: RetryPolicy): Promise<void> {
  return fetcher("/api/settings/retry-policy", { method: "PUT", body: policy });
}

// Publish settings (MAX_POSTS, MIN_DATE)
export interface PublishSettings {
  max_posts: number;
  min_date: string;
}

export async function fetchPublishSettings(): Promise<PublishSettings> {
  return fetcher<PublishSettings>("/api/settings/publish");
}

export async function updatePublishSettings(settings: PublishSettings): Promise<void> {
  return fetcher("/api/settings/publish", { method: "PUT", body: settings });
}