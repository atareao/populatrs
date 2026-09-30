import { describe, it, expect } from "vitest";
import { safeRedirectTarget } from "../utils/redirect";

describe("safeRedirectTarget", () => {
  it("returns an absolute https URL unchanged", () => {
    expect(safeRedirectTarget("https://idp.example.com/logout")).toBe(
      "https://idp.example.com/logout",
    );
  });

  it("returns an absolute http URL unchanged", () => {
    expect(safeRedirectTarget("http://idp.example.com/logout")).toBe(
      "http://idp.example.com/logout",
    );
  });

  it("falls back to /login for a javascript: URL", () => {
    expect(safeRedirectTarget("javascript:alert(1)")).toBe("/login");
  });

  it("falls back to /login for an empty string", () => {
    expect(safeRedirectTarget("")).toBe("/login");
  });

  it("falls back to /login for null", () => {
    expect(safeRedirectTarget(null)).toBe("/login");
  });

  it("falls back to /login for a relative URL", () => {
    expect(safeRedirectTarget("/logout")).toBe("/login");
  });

  it("falls back to /login for a data: URL", () => {
    expect(safeRedirectTarget("data:text/html,<script>alert(1)</script>")).toBe(
      "/login",
    );
  });

  it("returns the normalized URL (adds a trailing slash to a bare origin)", () => {
    expect(safeRedirectTarget("https://idp.example.com")).toBe(
      "https://idp.example.com/",
    );
  });

  it("normalizes the scheme to lowercase", () => {
    expect(safeRedirectTarget("HTTPS://idp.example.com/logout")).toBe(
      "https://idp.example.com/logout",
    );
  });
});
