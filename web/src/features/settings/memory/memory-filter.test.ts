import { describe, expect, it } from "vitest";
import type { MemorySummary } from "../../../api/contracts";
import {
  countMemories,
  EMPTY_MEMORY_FILTER,
  filterMemories,
  missingRationaleMarkers
} from "./memory-filter";

/** 构造一条记忆摘要。 */
function entry(partial: Partial<MemorySummary>): MemorySummary {
  return {
    name: "unnamed",
    description: "",
    type: "user",
    scope: "project",
    ...partial
  };
}

describe("filterMemories", () => {
  const entries = [
    entry({ name: "zh-writing", description: "中文书写规范", type: "feedback", scope: "global" }),
    entry({ name: "build-tools", description: "构建工具链", type: "project", scope: "project" }),
    entry({ name: "issue-board", description: "工单看板", type: "reference", scope: "global" }),
    entry({ name: "rust-user", description: "Rust 开发者", type: "user", scope: "project" })
  ];

  it("returns everything with the empty filter", () => {
    expect(filterMemories(entries, EMPTY_MEMORY_FILTER)).toHaveLength(4);
  });

  it("filters by type", () => {
    const names = filterMemories(entries, { ...EMPTY_MEMORY_FILTER, type: "feedback" }).map(
      (item) => item.name
    );
    expect(names).toEqual(["zh-writing"]);
  });

  it("filters by scope", () => {
    const names = filterMemories(entries, { ...EMPTY_MEMORY_FILTER, scope: "global" }).map(
      (item) => item.name
    );
    expect(names).toEqual(["zh-writing", "issue-board"]);
  });

  it("matches the keyword against name and description, case-insensitively", () => {
    // 标识命中
    expect(
      filterMemories(entries, { ...EMPTY_MEMORY_FILTER, query: "BUILD" }).map((item) => item.name)
    ).toEqual(["build-tools"]);
    // 摘要命中
    expect(
      filterMemories(entries, { ...EMPTY_MEMORY_FILTER, query: "看板" }).map((item) => item.name)
    ).toEqual(["issue-board"]);
  });

  it("combines type, scope and keyword", () => {
    const result = filterMemories(entries, {
      type: "reference",
      scope: "global",
      query: "看板"
    });
    expect(result.map((item) => item.name)).toEqual(["issue-board"]);
    expect(filterMemories(entries, { type: "reference", scope: "project", query: "" })).toHaveLength(0);
  });
});

describe("countMemories", () => {
  it("counts types and scopes independently of each other", () => {
    const counts = countMemories([
      entry({ type: "feedback", scope: "global" }),
      entry({ type: "feedback", scope: "project" }),
      entry({ type: "user", scope: "project" })
    ]);
    expect(counts.types).toEqual({ all: 3, user: 1, feedback: 2, project: 0, reference: 0 });
    expect(counts.scopes).toEqual({ all: 3, global: 1, project: 2 });
  });
});

describe("missingRationaleMarkers", () => {
  it("flags feedback and project missing both markers", () => {
    expect(missingRationaleMarkers("feedback", "一律使用 pnpm")).toEqual([
      "Why:",
      "How to apply:"
    ]);
    expect(missingRationaleMarkers("project", "目标")).toEqual(["Why:", "How to apply:"]);
  });

  it("reports only the marker that is actually missing", () => {
    expect(missingRationaleMarkers("project", "目标\n**Why:** 因为")).toEqual(["How to apply:"]);
  });

  it("never flags types without the requirement", () => {
    expect(missingRationaleMarkers("user", "用户是 Rust 开发者")).toEqual([]);
    expect(missingRationaleMarkers("reference", "看板：http://x")).toEqual([]);
  });
});
