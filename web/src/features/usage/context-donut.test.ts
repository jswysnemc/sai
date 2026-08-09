import { describe, expect, it } from "vitest";
import { donutArcs } from "./context-donut";

describe("donut arc geometry", () => {
  it("按份额换算弧长并顺时针衔接偏移", () => {
    const arcs = donutArcs([0.5, 0.3, 0.2]);

    expect(arcs.map((arc) => arc.length)).toEqual([50, 30, 20]);
    // 首段从 12 点开始，后续段偏移随累计弧长回退
    expect(arcs.map((arc) => arc.offset)).toEqual([25, -25, -55]);
  });

  it("零份额段弧长为零且不影响后续衔接", () => {
    const arcs = donutArcs([0.6, 0, 0.4]);

    expect(arcs[1].length).toBe(0);
    expect(arcs[2].offset).toBe(25 - 60);
  });

  it("超界份额被截断到 0 与 1 之间", () => {
    const arcs = donutArcs([1.4, -0.2]);

    expect(arcs[0].length).toBe(100);
    expect(arcs[1].length).toBe(0);
  });
});
