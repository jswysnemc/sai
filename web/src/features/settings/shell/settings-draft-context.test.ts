import { describe, expect, it, vi } from "vitest";
import { leavesSettingsDraft, type SettingsDraft } from "./settings-draft-context";

describe("独立设置文档离开保护", () => {
  const mcp: SettingsDraft = { scope: "/settings/mcp", dirty: true, saving: false, discard: vi.fn() };

  it("MCP 切换服务仍保留同一份草稿，离开分区才需确认", () => {
    expect(leavesSettingsDraft(mcp, { pathname: "/settings/mcp", search: "?item=other" })).toBe(false);
    expect(leavesSettingsDraft(mcp, { pathname: "/settings/git", search: "" })).toBe(true);
  });

  it("技能切换对象需确认，行为视图和字段定位仍属于同一份文档", () => {
    const skill = { ...mcp, scope: "/settings/skills", contains: (search: string) => new URLSearchParams(search).get("item") === "one" };
    expect(leavesSettingsDraft(skill, { pathname: skill.scope, search: "?item=one&view=behavior&focus=skills.enabled" })).toBe(false);
    expect(leavesSettingsDraft(skill, { pathname: skill.scope, search: "?item=two" })).toBe(true);
    expect(leavesSettingsDraft(skill, { pathname: skill.scope, search: "" })).toBe(true);
  });

  it("已保存的文档不拦截导航", () => {
    expect(leavesSettingsDraft({ ...mcp, dirty: false }, { pathname: "/", search: "" })).toBe(false);
  });
});
