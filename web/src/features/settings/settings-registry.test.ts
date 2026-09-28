import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS_SECTION,
  SETTINGS_GROUPS,
  SETTINGS_SECTIONS,
  dirtySettingsSections,
  filterSettingsSections,
  groupSettingsSections,
  resolveSettingsSectionId,
  resolveSettingsSubview,
  showsAppConfigSave
} from "./settings-registry";

describe("settings registry", () => {
  it("keeps unique section ids and known groups", () => {
    const ids = SETTINGS_SECTIONS.map((item) => item.id);
    expect(new Set(ids).size).toBe(ids.length);
    const groupIds = new Set(SETTINGS_GROUPS.map((item) => item.id));
    for (const section of SETTINGS_SECTIONS) {
      expect(groupIds.has(section.group)).toBe(true);
      expect(section.searchKeys.length).toBeGreaterThan(0);
    }
  });

  it("resolves route params with fallback", () => {
    expect(resolveSettingsSectionId(undefined)).toBe(DEFAULT_SETTINGS_SECTION);
    expect(resolveSettingsSectionId("mcp")).toBe("mcp");
    expect(resolveSettingsSectionId("plugins")).toBe("cli-tools");
    expect(resolveSettingsSectionId("not-a-section")).toBe(DEFAULT_SETTINGS_SECTION);
  });

  it("filters sections by bilingual keywords", () => {
    const byMcp = filterSettingsSections("mcp");
    expect(byMcp.some((item) => item.id === "mcp")).toBe(true);
    const byZh = filterSettingsSections("用量");
    expect(byZh.some((item) => item.id === "usage")).toBe(true);
    const bySessionData = filterSettingsSections("会话数据");
    expect(bySessionData.some((item) => item.id === "session-data")).toBe(true);
    const bySearchProvider = filterSettingsSections("tavily");
    expect(bySearchProvider.map((item) => item.id)).toEqual(["web-search"]);
    expect(resolveSettingsSectionId("web-search")).toBe("web-search");
  });

  it("groups sections and skips empty groups when filtered", () => {
    const grouped = groupSettingsSections(filterSettingsSections("gateway"));
    expect(grouped.every((entry) => entry.sections.length > 0)).toBe(true);
    expect(grouped.some((entry) => entry.group.id === "agentCapabilities")).toBe(true);
  });

  it("derives topbar save from the appConfig participation model", () => {
    // required 常驻保存；其余分区只在全局草稿有修改时露出
    expect(showsAppConfigSave("required", false)).toBe(true);
    expect(showsAppConfigSave("required", true)).toBe(true);
    expect(showsAppConfigSave("optional", false)).toBe(false);
    expect(showsAppConfigSave("optional", true)).toBe(true);
    expect(showsAppConfigSave("none", false)).toBe(false);
    expect(showsAppConfigSave("none", true)).toBe(true);
  });

  it("resolves subviews with fallback to the first page", () => {
    const runtime = SETTINGS_SECTIONS.find((item) => item.id === "runtime");
    expect(resolveSettingsSubview(runtime, "environment")).toBe("environment");
    expect(resolveSettingsSubview(runtime, "notifications")).toBe("execution");
    // 缺失或非法的子页段回落到首个子页
    expect(resolveSettingsSubview(runtime, undefined)).toBe("execution");
    expect(resolveSettingsSubview(runtime, "not-a-subview")).toBe("execution");
    // 无子页的分区始终返回 undefined
    const git = SETTINGS_SECTIONS.find((item) => item.id === "git");
    expect(resolveSettingsSubview(git, "anything")).toBeUndefined();
    expect(resolveSettingsSubview(undefined, "anything")).toBeUndefined();
    const usage = SETTINGS_SECTIONS.find((item) => item.id === "usage");
    expect(resolveSettingsSubview(usage, "logs")).toBe("logs");
  });

  it("maps legacy subview segments to the merged pages", () => {
    const runtime = SETTINGS_SECTIONS.find((item) => item.id === "runtime");
    expect(resolveSettingsSubview(runtime, "engine")).toBe("execution");
    expect(resolveSettingsSubview(runtime, "permissions")).toBe("environment");
    expect(resolveSettingsSubview(runtime, "terminal")).toBe("environment");
    expect(resolveSettingsSubview(runtime, "context")).toBe("tools");
    const usage = SETTINGS_SECTIONS.find((item) => item.id === "usage");
    expect(resolveSettingsSubview(usage, "providers")).toBe("breakdown");
    expect(resolveSettingsSubview(usage, "sessions")).toBe("breakdown");
    // Jev 合并为单页后旧子页段不再保留
    const jev = SETTINGS_SECTIONS.find((item) => item.id === "jev");
    expect(resolveSettingsSubview(jev, "connections")).toBeUndefined();
  });

  it("marks sections whose config paths differ from the saved snapshot", () => {
    const baseline = { git: { autofetch: false }, plugins: { web: { enabled: true }, memory: { enabled: true } } };
    const draft = { git: { autofetch: true }, plugins: { web: { enabled: false }, memory: { enabled: true } } };
    const dirty = dirtySettingsSections(draft, baseline);
    expect(dirty.has("git")).toBe(true);
    expect(dirty.has("web-search")).toBe(true);
    expect(dirty.has("memory")).toBe(false);
    expect(dirtySettingsSections(null, baseline).size).toBe(0);
  });

  it("gives every non-required section a bilingual save hint", () => {
    for (const section of SETTINGS_SECTIONS) {
      if (section.appConfig === "required") continue;
      expect(section.saveHintEn, section.id).toBeTruthy();
      expect(section.saveHintZh, section.id).toBeTruthy();
    }
  });
});
