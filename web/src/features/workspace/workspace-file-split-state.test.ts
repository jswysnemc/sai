import { describe, expect, it } from "vitest";
import {
  WORKSPACE_FILE_TREE_DEFAULT_WIDTH,
  WORKSPACE_FILE_TREE_MAX_WIDTH,
  WORKSPACE_FILE_TREE_MIN_WIDTH,
  clampWorkspaceFileTreeWidth,
  parseWorkspaceFileTreeWidth,
  shouldOverlayWorkspaceFileTree
} from "./workspace-file-split-state";

describe("workspace file split state", () => {
  it("为编辑器和文件树保留最小宽度", () => {
    expect(clampWorkspaceFileTreeWidth(80, 900)).toBe(WORKSPACE_FILE_TREE_MIN_WIDTH);
    expect(clampWorkspaceFileTreeWidth(700, 900)).toBe(WORKSPACE_FILE_TREE_MAX_WIDTH);
    expect(clampWorkspaceFileTreeWidth(360, 900)).toBe(360);
  });

  it("解析持久化宽度并拒绝无效值", () => {
    expect(parseWorkspaceFileTreeWidth("384")).toBe(384);
    expect(parseWorkspaceFileTreeWidth("invalid")).toBe(WORKSPACE_FILE_TREE_DEFAULT_WIDTH);
    expect(parseWorkspaceFileTreeWidth(null)).toBe(WORKSPACE_FILE_TREE_DEFAULT_WIDTH);
  });

  it("空间不足时切换为覆盖式文件树", () => {
    expect(shouldOverlayWorkspaceFileTree(539)).toBe(true);
    expect(shouldOverlayWorkspaceFileTree(540)).toBe(false);
  });
});
