import { describe, expect, it } from "vitest";
import type { DiffLine } from "./diff-model";
import { changeRowTone, changeTone, clampChangeOrdinal } from "./diff-change-blocks";

/**
 * 创建测试使用的最小差异行。
 *
 * @param kind 差异行类型
 * @returns 最小差异行对象
 */
function line(kind: DiffLine["kind"]): DiffLine {
  return { kind, text: "line" };
}

describe("diff change block helpers", () => {
  it("将导航序号限制在变更块范围内", () => {
    expect(clampChangeOrdinal(-1, 3)).toBe(0);
    expect(clampChangeOrdinal(9, 3)).toBe(2);
    expect(clampChangeOrdinal(2, 0)).toBe(0);
  });

  it("根据变更块内容确定整体色调", () => {
    expect(changeTone([{ left: line("removed"), right: null }])).toBe("removed");
    expect(changeTone([{ left: null, right: line("added") }])).toBe("added");
    expect(changeTone([{ left: line("removed"), right: line("added") }])).toBe("mixed");
    expect(changeTone([{ left: line("context"), right: line("context") }])).toBe("neutral");
  });

  it("为单行连接器保留方向信息", () => {
    expect(changeRowTone({ left: line("removed"), right: null })).toBe("removed");
    expect(changeRowTone({ left: null, right: line("added") })).toBe("added");
    expect(changeRowTone({ left: line("removed"), right: line("added") })).toBe("mixed");
  });
});
