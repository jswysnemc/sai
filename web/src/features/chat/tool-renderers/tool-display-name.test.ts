import { describe, expect, it } from "vitest";
import { text } from "../../i18n/locale";
import { readableToolName } from "../tool-lifecycle-card";
import { toolExposureTitle } from "./tool-display-name";

describe("tool display names", () => {
  it("工具卡种类名走界面语言", () => {
    const zh = (en: string, localized: string) => text("zh-CN", en, localized);
    const en = (english: string, localized: string) => text("en-US", english, localized);
    expect(readableToolName("write_file", false, zh)).toBe("写入");
    expect(readableToolName("write_file", false, en)).toBe("Write");
    expect(readableToolName("run_command", false, zh)).toBe("命令");
    expect(readableToolName("str_replace", false, zh)).toBe("替换");
    expect(readableToolName("background_command", true, zh)).toBe("任务");
    expect(readableToolName("background_command", true, en)).toBe("Tasks");
    expect(readableToolName("unknown_tool", false, zh)).toBe("unknown tool");
  });

  it("Jev 行使用工具名称而不是原始标识", () => {
    const zh = (en: string, localized: string) => text("zh-CN", en, localized);
    const en = (english: string, localized: string) => text("en-US", english, localized);
    expect(toolExposureTitle("context_status", zh)).toBe("查看上下文块");
    expect(toolExposureTitle("context_status", en)).toBe("Context Status");
    expect(toolExposureTitle("web_search", zh)).toBe("网页搜索");
    expect(toolExposureTitle("custom_tool", zh)).toBe("Custom Tool");
  });
});
