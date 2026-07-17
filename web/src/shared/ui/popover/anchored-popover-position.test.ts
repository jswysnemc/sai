import { describe, expect, it } from "vitest";
import { calculateAnchoredPopoverPosition } from "./anchored-popover-position";

describe("anchored popover position", () => {
  it("在移动端将左对齐菜单限制在视口内部", () => {
    expect(calculateAnchoredPopoverPosition(
      { left: 28, right: 142, top: 20, bottom: 48 },
      { viewportWidth: 375, viewportHeight: 800, preferredWidth: 520, minimumWidth: 240, align: "left" }
    )).toEqual({ left: 12, top: 54, width: 351, maxHeight: 420 });
  });

  it("将右对齐菜单贴近触发器并限制左边界", () => {
    expect(calculateAnchoredPopoverPosition(
      { left: 195, right: 286, top: 20, bottom: 48 },
      { viewportWidth: 375, viewportHeight: 800, preferredWidth: 220, minimumWidth: 180, align: "right" }
    )).toEqual({ left: 66, top: 54, width: 220, maxHeight: 420 });
  });

  it("底部空间不足时翻转到触发器上方", () => {
    const position = calculateAnchoredPopoverPosition(
      { left: 40, right: 200, top: 700, bottom: 730 },
      { viewportWidth: 900, viewportHeight: 800, preferredWidth: 320, minimumWidth: 240, align: "left", maxHeight: 400 }
    );
    expect(position.top).toBeUndefined();
    expect(position.bottom).toBe(106);
    expect(position.maxHeight).toBeLessThanOrEqual(400);
  });

  it("向下弹出时高度受剩余空间约束", () => {
    const position = calculateAnchoredPopoverPosition(
      { left: 40, right: 200, top: 80, bottom: 560 },
      { viewportWidth: 900, viewportHeight: 800, preferredWidth: 320, minimumWidth: 240, align: "left", maxHeight: 600 }
    );
    expect(position.top).toBe(566);
    expect(position.maxHeight).toBe(222);
  });
});
