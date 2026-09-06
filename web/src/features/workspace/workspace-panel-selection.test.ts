import { describe, expect, it } from "vitest";
import type { WorkspacePanelTab } from "./workspace-tab";
import { selectExistingWorkspacePanel } from "./workspace-panel-selection";

const tabs: WorkspacePanelTab[] = [
  { id: "readme", type: "files", title: "README.md", path: "README.md", closable: true },
  { id: "cargo", type: "files", title: "Cargo.toml", path: "Cargo.toml", closable: true },
  { id: "empty", type: "files", title: "Editor", closable: true },
  { id: "git", type: "diff", title: "Git", closable: true }
];

describe("工作区面板选择", () => {
  it("从 Git 返回第二个文件时保留用户选择", () => {
    expect(selectExistingWorkspacePanel(tabs, "files", "git", "Cargo.toml")?.id).toBe("cargo");
    expect(selectExistingWorkspacePanel(tabs, "files", "cargo", "Cargo.toml")?.id).toBe("cargo");
  });

  it("搜索选中的新文件优先于上一轮仍然激活的文件", () => {
    expect(selectExistingWorkspacePanel(tabs, "files", "readme", "Cargo.toml")?.id).toBe("cargo");
  });

  it("空编辑器和其他面板不会借用已选文件的标签", () => {
    expect(selectExistingWorkspacePanel(tabs, "files", "empty", null)?.id).toBe("empty");
    expect(selectExistingWorkspacePanel(tabs, "diff", "cargo", "Cargo.toml")?.id).toBe("git");
    expect(selectExistingWorkspacePanel(tabs, "tasks", "cargo", "Cargo.toml")).toBeUndefined();
  });
});
