import { describe, expect, it } from "vitest";
import { workspaceTabDirectoryLabels } from "./workspace-tab-labels";
import type { WorkspacePanelTab } from "./workspace-tab";

/**
 * 创建同名文件标签，供目录区分规则测试使用。
 * @param path 文件路径
 * @returns 可关闭的文件标签
 */
function fileTab(path: string): WorkspacePanelTab {
  return { id: path, type: "files", title: path.split(/[\\/]/).at(-1)!, path, closable: true };
}

describe("workspaceTabDirectoryLabels", () => {
  it("普通文件不增加重复路径信息", () => {
    expect(workspaceTabDirectoryLabels([fileTab("src/app.ts"), fileTab("README.md")]).size).toBe(0);
  });

  it("同名文件逐级补足目录，避免只显示相同的末级目录", () => {
    const paths = ["web/src/index.ts", "api/src/index.ts", "tests/index.ts"];
    const labels = workspaceTabDirectoryLabels(paths.map(fileTab));
    expect(paths.map((path) => labels.get(path))).toEqual(["web/src", "api/src", "tests"]);
  });

  it("区分根目录文件，并兼容 Windows 路径", () => {
    const paths = ["README.md", "docs/README.md", "C:\\repo\\examples\\README.md"];
    const labels = workspaceTabDirectoryLabels(paths.map(fileTab));
    expect(paths.map((path) => labels.get(path))).toEqual(["./", "docs", "examples"]);
  });

  it("同一文件的编辑标签与差异标签不产生无用目录说明", () => {
    const file = fileTab("src/app.ts");
    expect(workspaceTabDirectoryLabels([file, { ...file, id: "diff:src/app.ts", type: "diff" }]).size).toBe(0);
  });
});
