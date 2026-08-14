import { describe, expect, it } from "vitest";
import { countByTurn, filterRecords } from "./trajectory-filter";
import type { TrajectoryRecord, TrajectoryRecordKind } from "./trajectory-record";

/**
 * 构造测试记录。
 *
 * @param overrides 需要覆盖的字段
 * @returns 轨迹记录
 */
function record(overrides: Partial<TrajectoryRecord> & { id: string }): TrajectoryRecord {
  return {
    index: 1,
    kind: "tool",
    turnId: "t1",
    turnSeq: 1,
    turnStart: false,
    round: 1,
    roundStart: false,
    summary: "",
    label: null,
    startedAt: 1000,
    durationMs: 100,
    failed: false,
    running: false,
    detail: {},
    ...overrides
  };
}

/** 不隐藏任何种类。 */
const NONE: ReadonlySet<TrajectoryRecordKind> = new Set();
/** 没有折叠任何轮次。 */
const NO_TURNS: ReadonlySet<string> = new Set();

describe("filterRecords", () => {
  it("搜索词命中详情里的工具输出", () => {
    const records = [
      record({ id: "a", summary: "读取配置", detail: { output: "listen_port = 8080" } }),
      record({ id: "b", summary: "写入文件", detail: { output: "ok" } })
    ];
    const kept = filterRecords(records, {
      query: "8080",
      hiddenKinds: NONE,
      range: null,
      collapsedTurns: NO_TURNS
    });
    expect(kept.map((item) => item.id)).toEqual(["a"]);
  });

  it("隐藏的种类不出现在结果里", () => {
    const records = [record({ id: "a", kind: "tool" }), record({ id: "b", kind: "user" })];
    const kept = filterRecords(records, {
      query: "",
      hiddenKinds: new Set<TrajectoryRecordKind>(["tool"]),
      range: null,
      collapsedTurns: NO_TURNS
    });
    expect(kept.map((item) => item.id)).toEqual(["b"]);
  });

  it("时间区间保留与之重叠的长记录", () => {
    const records = [
      record({ id: "cross", startedAt: 0, durationMs: 5000 }),
      record({ id: "outside", startedAt: 9000, durationMs: 10 })
    ];
    const kept = filterRecords(records, {
      query: "",
      hiddenKinds: NONE,
      range: { from: 4000, to: 6000 },
      collapsedTurns: NO_TURNS
    });
    expect(kept.map((item) => item.id)).toEqual(["cross"]);
  });

  it("折叠的轮次只保留首条记录", () => {
    const records = [
      record({ id: "head", turnStart: true }),
      record({ id: "body" }),
      record({ id: "other", turnId: "t2" })
    ];
    const kept = filterRecords(records, {
      query: "",
      hiddenKinds: NONE,
      range: null,
      collapsedTurns: new Set(["t1"])
    });
    expect(kept.map((item) => item.id)).toEqual(["head", "other"]);
  });
});

describe("countByTurn", () => {
  it("按轮次统计记录数", () => {
    const counts = countByTurn([
      record({ id: "a" }),
      record({ id: "b" }),
      record({ id: "c", turnId: "t2" }),
      record({ id: "d", turnId: null })
    ]);
    expect(counts.get("t1")).toBe(2);
    expect(counts.get("t2")).toBe(1);
    expect(counts.size).toBe(2);
  });
});
