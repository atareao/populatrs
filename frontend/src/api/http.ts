export interface User {
  sub: string;
  email: string;
  name: string;
}

export interface FeedConfig {
  id: string;
  type: string;
  config: Record<string, unknown>;
  name: string;
  enabled: boolean;
  publishers: string[];
  check_interval_minutes?: number;
  max_retries?: number;
  retry_delay_seconds?: number;
}

export interface PublisherConfigEntry {
  type: string;
  config: Record<string, unknown>;
  enabled: boolean;
}

export interface ScheduleConfig {
  default_interval_minutes: number;
  timezone: string;
}

export interface StorageConfig {
  data_dir: string;
}

export interface DashboardStatus {
  feeds: { total: number; enabled: number; disabled: number };
  publishers: { total: number };
  published_posts: number;
  schedule: { interval_minutes: number; timezone: string };
  storage: { data_dir: string };
}

import { getToken } from "../store/auth";

async function fetcher<T>(path: string, opts?: { method?: string; body?: unknown }): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(path, {
    method: opts?.method ?? (opts?.body ? "POST" : "GET"),
    headers,
    body: opts?.body ? JSON.stringify(opts.body) : undefined,
  });

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
export async function runFeed(id: string): Promise<{ posts_count: number; posts: unknown[] }> {
  return fetcher(`/api/feeds/${id}/run`, { method: "POST" });
}

// OAuth
export async function getOAuthUrl(id: string): Promise<{ url: string }> {
  return fetcher(`/api/publishers/${id}/oauth/authorize`);
}

export async function oauthCallback(id: string, code: string, state: string): Promise<{ success: boolean; message: string }> {
  return fetcher(`/api/publishers/${id}/oauth/callback`, {
    method: "POST",
    body: { code, state },
  });
}

// Status
export async function fetchStatus(): Promise<DashboardStatus> {
  return fetcher("/api/status");
}