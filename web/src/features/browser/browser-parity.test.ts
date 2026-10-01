import { describe, expect, it } from "vitest";
import { formatPickedElement, pickedElementLabel } from "./browser-element-context";
import { parseBrowserServerMessage } from "./browser-protocol";
import { displayScale } from "./browser-responsive";

describe("浏览器面板交互协议", () => {
  it("解析对话框、下拉菜单、文件选择与下载消息，关闭消息允许为空", () => {
    expect(parseBrowserServerMessage('{"type":"dialog","dialog":{"kind":"confirm","message":"ok?","default_prompt":"","url":"https://a.b"}}'))
      .toMatchObject({ type: "dialog", dialog: { kind: "confirm" } });
    expect(parseBrowserServerMessage('{"type":"dialog","dialog":null}')).toEqual({ type: "dialog", dialog: null });
    expect(parseBrowserServerMessage('{"type":"file_chooser","chooser":null}')).toEqual({ type: "file_chooser", chooser: null });
    expect(parseBrowserServerMessage('{"type":"picked","element":null}')).toEqual({ type: "picked", element: null });
    expect(parseBrowserServerMessage('{"type":"select_popup","popup":{"options":[],"selected":0,"x":1,"y":2,"width":3,"height":4}}'))
      .toMatchObject({ type: "select_popup", popup: { x: 1 } });
    expect(parseBrowserServerMessage('{"type":"select_popup","popup":null}')).toBeNull();
    expect(parseBrowserServerMessage('{"type":"clipboard","text":"hi"}')).toEqual({ type: "clipboard", text: "hi" });
    expect(parseBrowserServerMessage('{"type":"info","devtools_url":null,"persistent_profile":true}'))
      .toEqual({ type: "info", devtools_url: null, persistent_profile: true });
  });

  it("选中的元素整理为带定位信息的上下文，标签带上元素 id", () => {
    const element = {
      pageUrl: "https://shop.dev/cart",
      pageTitle: "Cart",
      tagName: "button",
      role: "button",
      selector: "#buy",
      text: "Buy now",
      attributes: { id: "buy", class: "primary" },
      rect: { x: 10, y: 20, width: 120, height: 40 },
      style: { color: "#FFFFFF", fontSize: "14px" },
      htmlExcerpt: "<button id=\"buy\">Buy now</button>"
    };
    const text = formatPickedElement(element);
    expect(text).toContain("URL: https://shop.dev/cart");
    expect(text).toContain("Selector: #buy");
    expect(text).toContain("Box: 120x40 at (10, 20)");
    expect(text).toContain("color: #FFFFFF");
    expect(text).toContain("```html");
    expect(pickedElementLabel(element)).toBe("Browser · button#buy");
    expect(pickedElementLabel({ ...element, attributes: {} })).toBe("Browser · button");
  });

  it("自由尺寸按缩放档位显示，适应面板时只缩小不放大", () => {
    const page = { width: 390, height: 844 };
    expect(displayScale(page, { width: 800, height: 422 }, "fit")).toBeCloseTo(0.5);
    expect(displayScale(page, { width: 2000, height: 2000 }, "fit")).toBe(1);
    expect(displayScale(page, { width: 100, height: 100 }, 0.75)).toBe(0.75);
    expect(displayScale(page, null, "fit")).toBe(1);
  });
});
