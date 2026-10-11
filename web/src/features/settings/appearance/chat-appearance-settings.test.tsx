import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ChatAppearanceSettings } from "./chat-appearance-settings";

describe("Chat appearance settings", () => {
  it("renders font, size, width controls and a live markdown preview", () => {
    const html = renderToStaticMarkup(<ChatAppearanceSettings />);
    expect(html).toContain("聊天阅读");
    expect(html).toContain("对话字体");
    expect(html).toContain("系统");
    expect(html).toContain("无衬线");
    expect(html).toContain("衬线");
    expect(html).toContain("等宽");
    expect(html).toContain("对话字号");
    expect(html).toContain("12–24");
    expect(html).toContain("对话列宽");
    expect(html).toContain("标准");
    expect(html).toContain("宽栏");
    expect(html).toContain("铺满");
    expect(html).toContain("恢复默认");
    expect(html).toContain("字体预览");
    expect(html).toContain("chat-reading-markdown");
    expect(html).toContain("更清晰的对话");
    expect(html).toContain("50rem");
    expect(html).toContain("15px");
  });
});
