import { describe, expect, it } from "vitest";
import {
  consumePendingBrowserNavigation,
  requestBuiltInBrowserNavigation,
  subscribeBrowserNavigation
} from "./browser-preview-navigation";

describe("built-in browser preview navigation", () => {
  it("keeps the address until the panel connects, then delivers later requests live", () => {
    requestBuiltInBrowserNavigation("http://127.0.0.1/first.html");
    expect(consumePendingBrowserNavigation()).toBe("http://127.0.0.1/first.html");
    expect(consumePendingBrowserNavigation()).toBeNull();

    const seen: string[] = [];
    const unsubscribe = subscribeBrowserNavigation((url) => seen.push(url));
    requestBuiltInBrowserNavigation("http://127.0.0.1/second.html");
    unsubscribe();
    requestBuiltInBrowserNavigation("http://127.0.0.1/third.html");

    expect(seen).toEqual(["http://127.0.0.1/second.html"]);
    expect(consumePendingBrowserNavigation()).toBe("http://127.0.0.1/third.html");
  });
});
