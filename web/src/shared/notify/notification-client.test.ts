import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchNotificationPlan } from "./notification-client";

afterEach(() => { vi.unstubAllGlobals(); });

describe("notification policy API", () => {
  it("sends only status and locale through the authenticated API", async () => {
    const notifications = [{ title: "Sai", body: "由插件提供", desktop: false, sound: true }];
    const fetcher = vi.fn(async () => new Response(JSON.stringify({ notifications })));
    vi.stubGlobal("fetch", fetcher);
    const signal = new AbortController().signal;
    expect(await fetchNotificationPlan("failed", "zh-CN", signal)).toEqual(notifications);
    expect(fetcher).toHaveBeenCalledWith("/api/notifications/plan", {
      credentials: "same-origin", method: "POST", signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ status: "failed", locale: "zh-CN" })
    });
  });

  it.each([
    {},
    { notifications: null },
    { notifications: [{ title: "Sai", body: "body" }] },
    { notifications: [{ title: "Sai", body: "body", desktop: "false", sound: false }] },
    { notifications: Array(9).fill({ title: "Sai", body: "body", desktop: true, sound: true }) }
  ])("rejects an incomplete or oversized plan without applying default switches", async (body) => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(body))));
    await expect(fetchNotificationPlan("completed", "en-US", new AbortController().signal))
      .rejects.toThrow("Invalid notification plan");
  });

  it("preserves a disabled plugin's empty plan", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response('{"notifications":[]}')));
    expect(await fetchNotificationPlan("completed", "en-US", new AbortController().signal)).toEqual([]);
  });
});
