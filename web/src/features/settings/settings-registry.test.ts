import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS_SECTION,
  SETTINGS_GROUPS,
  SETTINGS_SECTIONS,
  filterSettingsSections,
  groupSettingsSections,
  resolveSettingsSectionId,
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
    expect(bySearchProvider.some((item) => item.id === "web-search")).toBe(true);
  });

  it("groups sections and skips empty groups when filtered", () => {
    const grouped = groupSettingsSections(filterSettingsSections("gateway"));
    expect(grouped.every((entry) => entry.sections.length > 0)).toBe(true);
    expect(grouped.some((entry) => entry.group.id === "integrations")).toBe(true);
  });

  it("derives topbar save from the appConfig participation model", () => {
    // required 常驻保存；optional 仅脏时露出；none 永不展示
    expect(showsAppConfigSave("required", false)).toBe(true);
    expect(showsAppConfigSave("required", true)).toBe(true);
    expect(showsAppConfigSave("optional", false)).toBe(false);
    expect(showsAppConfigSave("optional", true)).toBe(true);
    expect(showsAppConfigSave("none", false)).toBe(false);
    expect(showsAppConfigSave("none", true)).toBe(false);
  });

  it("gives every non-required section a bilingual save hint", () => {
    for (const section of SETTINGS_SECTIONS) {
      if (section.appConfig === "required") continue;
      expect(section.saveHintEn, section.id).toBeTruthy();
      expect(section.saveHintZh, section.id).toBeTruthy();
    }
  });
});
