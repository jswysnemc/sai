import { describe, expect, it } from "vitest";
import type { TrajectoryRecord } from "./trajectory-record";
import { panDomain, trackSegments, trajectoryDomain, zoomDomain } from "./trajectory-scale";

/**
 * 构造只含时序字段的测试记录。
 *
 * @param id 记录标识
 * @param startedAt 起始毫秒
 * @param durationMs 耗时毫秒
 * @returns 轨迹记录
 */
function record(id: string, startedAt: number | null, durationMs: number | null): TrajectoryRecord {
  return {
    id,
    index: 1,
    kind: "tool",
    turnId: "t1",
    turnSeq: 1,
    turnStart: false,
    round: 1,
    roundStart: false,
    summary: "",
    label: null,
    startedAt,
    durationMs,
    failed: false,
    running: false,
    detail: {}
  };
}

describe("trajectoryDomain", () => {
  it("覆盖最早起点到最晚终点", () => {
    expect(trajectoryDomain([record("a", 1000, 500), record("b", 2000, 1000)]))
      .toEqual({ from: 1000, to: 3000 });
  });

  it("跳过没有时刻的记录", () => {
    expect(trajectoryDomain([record("a", null, null), record("b", 5000, 0)]))
      .toEqual({ from: 5000, to: 5001 });
  });

  it("没有任何时刻时返回 null", () => {
    expect(trajectoryDomain([record("a", null, null)])).toBeNull();
  });
});

describe("trackSegments", () => {
  it("真实耗时口径按时间比例投影", () => {
    const segments = trackSegments(
      [record("a", 0, 500), record("b", 500, 500)],
      "duration",
      { from: 0, to: 1000 }
    );
    expect(segments.map((segment) => [segment.left, segment.width])).toEqual([[0, 50], [50, 50]]);
  });

  it("零耗时记录退化为最小宽度刻线", () => {
    const [segment] = trackSegments([record("a", 500, 0)], "duration", { from: 0, to: 1000 });
    expect(segment.instant).toBe(true);
    expect(segment.width).toBeCloseTo(0.4);
  });

  it("缺少时刻的记录不投影到轨道上", () => {
    expect(trackSegments([record("a", null, null)], "duration", { from: 0, to: 1000 })).toEqual([]);
  });

  it("顺序口径下每条记录等宽", () => {
    const segments = trackSegments(
      [record("a", null, null), record("b", 100, 900), record("c", 1000, 0)],
      "sequence",
      null
    );
    expect(segments.map((segment) => segment.width)).toEqual([100 / 3, 100 / 3, 100 / 3]);
  });
});

describe("zoomDomain", () => {
  const bounds = { from: 0, to: 1000 };

  it("放大时锚点保持在原比例位置", () => {
    const next = zoomDomain(bounds, bounds, 0.5, 0.5);
    expect(next).toEqual({ from: 250, to: 750 });
  });

  it("缩小不会越过边界", () => {
    expect(zoomDomain({ from: 400, to: 600 }, bounds, 0.5, 100)).toEqual(bounds);
  });

  it("靠近左边界放大时视窗贴边而不越界", () => {
    const next = zoomDomain(bounds, bounds, 0, 0.5);
    expect(next).toEqual({ from: 0, to: 500 });
  });
});

describe("panDomain", () => {
  const bounds = { from: 0, to: 1000 };

  it("按视窗宽度比例平移", () => {
    expect(panDomain({ from: 200, to: 400 }, bounds, 0.5)).toEqual({ from: 300, to: 500 });
  });

  it("平移到边界后停住", () => {
    expect(panDomain({ from: 800, to: 1000 }, bounds, 0.5)).toEqual({ from: 800, to: 1000 });
  });
});
