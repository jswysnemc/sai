import { describe, expect, it } from "vitest";
import {
  countToolModes,
  DEFERRED_ALL_NON_BASE,
  expandWildcard,
  resolveToolMode,
  updateToolModes
} from "./agent-tool-mode-state";

describe("resolveToolMode", () => {
  it("按启用与延迟集合判定三段状态", () => {
    const selection = { enabled: ["read_file", "web_search"], deferred: ["web_search"] };

    expect(resolveToolMode(selection, "read_file", true)).toBe("on");
    expect(resolveToolMode(selection, "web_search", false)).toBe("load");
    expect(resolveToolMode(selection, "show_meme", false)).toBe("off");
  });

  it("白名单为空时代表全量开放", () => {
    const selection = { enabled: [], deferred: ["deep_research"] };

    expect(resolveToolMode(selection, "show_meme", false)).toBe("on");
    expect(resolveToolMode(selection, "deep_research", false)).toBe("load");
  });

  it("通配符只覆盖非基础工具", () => {
    const selection = { enabled: [], deferred: [DEFERRED_ALL_NON_BASE] };

    expect(resolveToolMode(selection, "read_file", true)).toBe("on");
    expect(resolveToolMode(selection, "web_search", false)).toBe("load");
  });
});

describe("updateToolModes", () => {
  const allNames = ["read_file", "web_search", "show_meme"];

  it("切到 load 时同时写入两个集合", () => {
    const next = updateToolModes(
      { enabled: ["read_file", "web_search"], deferred: [] },
      ["web_search"],
      "load",
      allNames
    );

    expect(next.enabled).toContain("web_search");
    expect(next.deferred).toEqual(["web_search"]);
  });

  it("切到 on 时从延迟集合移除", () => {
    const next = updateToolModes(
      { enabled: ["read_file", "web_search"], deferred: ["web_search"] },
      ["web_search"],
      "on",
      allNames
    );

    expect(next.enabled).toContain("web_search");
    expect(next.deferred).toEqual([]);
  });

  it("切到 off 时从两个集合同时移除", () => {
    const next = updateToolModes(
      { enabled: ["read_file", "web_search"], deferred: ["web_search"] },
      ["web_search"],
      "off",
      allNames
    );

    expect(next.enabled).toEqual(["read_file"]);
    expect(next.deferred).toEqual([]);
  });

  it("全量开放下关闭单个工具会展开显式白名单", () => {
    const next = updateToolModes({ enabled: [], deferred: [] }, ["show_meme"], "off", allNames);

    expect(next.enabled).toEqual(["read_file", "web_search"]);
  });

  it("批量更新保持顺序且不产生重复", () => {
    const next = updateToolModes(
      { enabled: ["read_file"], deferred: [] },
      ["web_search", "web_search", "show_meme"],
      "load",
      allNames
    );

    expect(next.enabled).toEqual(["read_file", "web_search", "show_meme"]);
    expect(next.deferred).toEqual(["web_search", "show_meme"]);
  });
});

describe("expandWildcard", () => {
  it("把通配符改写为逐项列出的具体工具名", () => {
    const next = expandWildcard(
      { enabled: [], deferred: [DEFERRED_ALL_NON_BASE, "web_search"] },
      ["web_search", "show_meme"]
    );

    expect(next.deferred).toEqual(["web_search", "show_meme"]);
  });

  it("没有通配符时保持原样", () => {
    const selection = { enabled: [], deferred: ["web_search"] };

    expect(expandWildcard(selection, ["web_search", "show_meme"])).toBe(selection);
  });
});

describe("countToolModes", () => {
  it("统计一组工具中各状态的数量", () => {
    const counts = countToolModes(
      { enabled: ["read_file", "web_search"], deferred: ["web_search"] },
      ["read_file", "web_search", "show_meme"],
      (name) => name === "read_file"
    );

    expect(counts).toEqual({ on: 1, load: 1, off: 1 });
  });
});
