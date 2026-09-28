import { describe, expect, it } from "vitest";
import { SETTINGS_SECTIONS } from "../settings-registry";
import { fieldAnchorId } from "./field-anchor";
import { SETTINGS_SEARCH_INDEX } from "./settings-search-index";
import { normalizeSearchText, scoreSearchEntry, searchEntryHref, searchSettingsFields } from "./settings-search";
import type { SettingsSearchEntry } from "./settings-search-types";

const SAMPLE: SettingsSearchEntry[] = [
  {
    anchor: "runtime.context.compaction_ratio",
    section: "runtime",
    subview: "tools",
    labelEn: "Auto-compact ratio",
    labelZh: "自动压缩比例",
    keywords: ["context.compaction_ratio", "压缩"]
  },
  {
    anchor: "providers.connection.api_key",
    section: "providers",
    subview: "connection",
    labelEn: "API keys",
    labelZh: "接口密钥",
    keywords: ["api_key", "密钥", "token"]
  }
];

describe("settings field search", () => {
  it("normalizes separators so api key variants match", () => {
    expect(normalizeSearchText("API key")).toBe("apikey");
    expect(normalizeSearchText("api_key")).toBe("apikey");
    expect(normalizeSearchText("context.compaction-ratio")).toBe("contextcompactionratio");
  });

  it("ranks label matches above keyword matches", () => {
    const [ratio, keys] = SAMPLE;
    expect(scoreSearchEntry(ratio, normalizeSearchText("自动压缩比例"))).toBe(100);
    expect(scoreSearchEntry(ratio, normalizeSearchText("自动"))).toBe(80);
    expect(scoreSearchEntry(keys, normalizeSearchText("api_key"))).toBeGreaterThan(0);
    expect(scoreSearchEntry(ratio, normalizeSearchText("compaction_ratio"))).toBe(30);
    expect(scoreSearchEntry(ratio, normalizeSearchText("温度"))).toBe(0);
  });

  it("returns localized labels with the section path", () => {
    const hits = searchSettingsFields("压缩", "zh-CN", SAMPLE);
    expect(hits).toHaveLength(1);
    expect(hits[0].label).toBe("自动压缩比例");
    expect(hits[0].location).toBe("运行时 › 工具与上下文");
    expect(searchSettingsFields("  ", "zh-CN", SAMPLE)).toEqual([]);
  });

  it("builds a focus link to the owning page", () => {
    expect(searchEntryHref(SAMPLE[0])).toBe("/settings/runtime/tools?focus=runtime.context.compaction_ratio");
    expect(fieldAnchorId(SAMPLE[0].anchor)).toBe("settings-field-runtime-context-compaction_ratio");
  });

  it("keeps the global index consistent with the registry", () => {
    const anchors = new Set<string>();
    for (const entry of SETTINGS_SEARCH_INDEX) {
      const section = SETTINGS_SECTIONS.find((item) => item.id === entry.section);
      expect(section, entry.anchor).toBeDefined();
      if (entry.subview) {
        expect(section?.subviews?.some((item) => item.id === entry.subview), entry.anchor).toBe(true);
      } else {
        expect(section?.subviews?.length ?? 0, entry.anchor).toBe(0);
      }
      expect(anchors.has(entry.anchor), `${entry.anchor} is duplicated`).toBe(false);
      anchors.add(entry.anchor);
    }
  });
});
