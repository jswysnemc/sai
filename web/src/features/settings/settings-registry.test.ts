import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS_SECTION,
  SETTINGS_GROUPS,
  SETTINGS_SECTIONS,
  filterSettingsSections,
  groupSettingsSections,
  resolveSettingsSectionId,
  showsGlobalAppConfigSave
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
    expect(resolveSettingsSectionId("not-a-section")).toBe(DEFAULT_SETTINGS_SECTION);
  });

  it("filters sections by bilingual keywords", () => {
    const byMcp = filterSettingsSections("mcp");
    expect(byMcp.some((item) => item.id === "mcp")).toBe(true);
    const byZh = filterSettingsSections("用量");
    expect(byZh.some((item) => item.id === "usage")).toBe(true);
  });

  it("groups sections and skips empty groups when filtered", () => {
    const grouped = groupSettingsSections(filterSettingsSections("gateway"));
    expect(grouped.every((entry) => entry.sections.length > 0)).toBe(true);
    expect(grouped.some((entry) => entry.group.id === "integrations")).toBe(true);
  });

  it("shows global save only for app-config surfaces", () => {
    expect(showsGlobalAppConfigSave("app-config")).toBe(true);
    expect(showsGlobalAppConfigSave("local-config")).toBe(false);
    expect(showsGlobalAppConfigSave("client-pref")).toBe(false);
    expect(showsGlobalAppConfigSave("operations")).toBe(false);
    expect(showsGlobalAppConfigSave("analytics")).toBe(false);
  });
});
