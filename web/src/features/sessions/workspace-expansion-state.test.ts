import { describe, expect, it } from "vitest";
import {
  parseExpandedWorkspaces,
  toggleWorkspaceExpanded,
  withActiveWorkspaceExpanded
} from "./workspace-expansion-state";

describe("workspace expansion", () => {
  it("损坏的存储不会展开任何工作区", () => {
    expect(parseExpandedWorkspaces("not-json").size).toBe(0);
    expect([...parseExpandedWorkspaces(JSON.stringify(["a", 1, ""]))]).toEqual(["a"]);
  });

  it("活动工作区自动展开，已展开的选择保留", () => {
    const expanded = toggleWorkspaceExpanded(new Set(["other"]), "other");
    expect(expanded.has("other")).toBe(false);
    const withActive = withActiveWorkspaceExpanded(new Set(["other"]), "current");
    expect([...withActive].sort()).toEqual(["current", "other"]);
    expect(withActiveWorkspaceExpanded(withActive, "current")).toEqual(withActive);
  });
});
