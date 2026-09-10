import { afterEach, describe, expect, it, vi } from "vitest";
import { deliverNotification } from "./notification-delivery";

afterEach(() => { vi.unstubAllGlobals(); });

describe("notification platform delivery", () => {
  it("preserves the plugin's full Unicode text and independent desktop switch", () => {
    const shown = vi.fn();
    class BrowserNotification {
      static permission = "granted";
      constructor(title: string, options: NotificationOptions) { shown(title, options); }
    }
    vi.stubGlobal("Notification", BrowserNotification);
    vi.stubGlobal("window", { Notification: BrowserNotification });
    const body = "𠀀".repeat(240);
    const signal = new AbortController().signal;
    deliverNotification({ title: "Lua title", body, desktop: true, sound: false }, signal);
    deliverNotification({ title: "hidden", body, desktop: false, sound: false }, signal);
    expect(shown).toHaveBeenCalledExactlyOnceWith("Lua title", { body, silent: true });
  });

  it("does not show a notification after its permission result becomes stale", async () => {
    const shown = vi.fn();
    let permit!: (permission: NotificationPermission) => void;
    class BrowserNotification {
      static permission = "default";
      static requestPermission = () => new Promise<NotificationPermission>((resolve) => { permit = resolve; });
      constructor() { shown(); }
    }
    vi.stubGlobal("Notification", BrowserNotification);
    vi.stubGlobal("window", { Notification: BrowserNotification });
    const controller = new AbortController();
    deliverNotification({ title: "Sai", body: "body", desktop: true, sound: false }, controller.signal);
    controller.abort();
    permit("granted");
    await Promise.resolve();
    expect(shown).not.toHaveBeenCalled();
  });

  it("isolates permission promise rejection", async () => {
    class BrowserNotification {
      static permission = "default";
      static requestPermission = () => Promise.reject(new Error("denied"));
    }
    vi.stubGlobal("Notification", BrowserNotification);
    vi.stubGlobal("window", { Notification: BrowserNotification });
    expect(() => deliverNotification(
      { title: "Sai", body: "body", desktop: true, sound: false }, new AbortController().signal
    )).not.toThrow();
    await Promise.resolve();
    await Promise.resolve();
  });
});
