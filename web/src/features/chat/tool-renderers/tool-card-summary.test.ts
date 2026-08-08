import { describe, expect, it } from "vitest";
import { toolCardSummary } from "./tool-card-summary";

describe("tool card summary", () => {
  it("展示简化后的 Shell 命令和加载目标", () => {
    expect(toolCardSummary("run_command", JSON.stringify({ command: "git status --short" }))).toBe("git status --short");
    expect(toolCardSummary("run_command", JSON.stringify({ command: "semble search foo bar" }))).toBe("semble search foo");
    expect(toolCardSummary("load", JSON.stringify({ skill_name: "drawio" }))).toBe("drawio");
    expect(toolCardSummary("load", JSON.stringify({ tool_names: ["web_search", "web_fetch"] }))).toBe("web_search, web_fetch");
    expect(toolCardSummary("load", JSON.stringify({ type: "skill", keywords: ["cc-switch-ops"] }))).toBe("skill · cc-switch-ops");
  });

  it("展示批量读取的首个路径和数量", () => {
    const argumentsText = JSON.stringify({ files: [{ path: "src/a.ts" }, { path: "src/b.ts" }] });
    expect(toolCardSummary("read_file", argumentsText)).toBe("src/a.ts 等");
  });

  it("工作区内路径优先展示相对路径", () => {
    expect(
      toolCardSummary(
        "read_file",
        JSON.stringify({ path: "/home/snemc/workspace/sai/src/main.rs" }),
        "zh-CN",
        "/home/snemc/workspace/sai"
      )
    ).toBe("src/main.rs");
  });

  it("复杂正则不直接展示", () => {
    const messy = "执行了|个命令|ran \\d+|commands? executed|tool.?fold";
    expect(toolCardSummary("grep", JSON.stringify({ pattern: messy, path: "web/src" }))).toBe("web/src");
    expect(toolCardSummary("grep", JSON.stringify({ pattern: messy }))).toBe("代码搜索");
  });

  it("展示 AUR 审查和安装工具的包名", () => {
    expect(toolCardSummary("review_aur_package", JSON.stringify({ package: "visual-studio-code-bin" }))).toBe("visual-studio-code-bin");
    expect(toolCardSummary("install_aur_package", JSON.stringify({ package: "paru", user_confirmed: true }))).toBe("paru");
  });

  it("展示 Trash 工具的目标路径", () => {
    expect(
      toolCardSummary(
        "trash_path",
        JSON.stringify({ path: "/home/snemc/workspace/tmp/sandbox/ball-battle" }),
        "zh-CN",
        "/home/snemc/workspace/tmp/sandbox"
      )
    ).toBe("ball-battle");
  });

  it("兼容尚未形成合法 JSON 的参数预览", () => {
    expect(toolCardSummary("custom_tool", "first\n  second")).toBe("first second");
  });
});
