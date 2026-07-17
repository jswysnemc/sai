import { describe, expect, it } from "vitest";
import { toolCardSummary } from "./tool-card-summary";

describe("tool card summary", () => {
  it("展示 Shell 命令和加载目标", () => {
    expect(toolCardSummary("run_command", JSON.stringify({ command: "git status --short" }))).toBe("git status --short");
    expect(toolCardSummary("load", JSON.stringify({ skill_name: "drawio" }))).toBe("drawio");
  });

  it("展示批量读取的首个路径和数量", () => {
    const argumentsText = JSON.stringify({ files: [{ path: "src/a.ts" }, { path: "src/b.ts" }] });
    expect(toolCardSummary("read_file", argumentsText)).toBe("src/a.ts 等 2 项");
  });

  it("展示 AUR 审查和安装工具的包名", () => {
    expect(toolCardSummary("review_aur_package", JSON.stringify({ package: "visual-studio-code-bin" }))).toBe("visual-studio-code-bin");
    expect(toolCardSummary("install_aur_package", JSON.stringify({ package: "paru", user_confirmed: true }))).toBe("paru");
  });

  it("兼容尚未形成合法 JSON 的参数预览", () => {
    expect(toolCardSummary("custom_tool", "first\n  second")).toBe("first second");
  });
});
