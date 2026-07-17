import { describe, expect, it } from "vitest";
import type { TodoItem, TodoStatus } from "../../api/contracts";
import { summarizeTodos } from "./todo-summary";

/**
 * 构造测试用 TODO 项。
 *
 * @param status 状态
 * @param text 文本
 * @returns TODO 项
 */
function todo(status: TodoStatus, text: string): TodoItem {
  return { id: `${status}-${text}`, text, status, created_at: "", updated_at: "" };
}

describe("summarizeTodos", () => {
  it("空清单返回零进度", () => {
    const summary = summarizeTodos([]);
    expect(summary.total).toBe(0);
    expect(summary.ratio).toBe(0);
    expect(summary.allDone).toBe(false);
    expect(summary.activeText).toBeNull();
  });

  it("按完成与取消统计进度比例", () => {
    const summary = summarizeTodos([
      todo("completed", "a"),
      todo("cancelled", "b"),
      todo("in_progress", "c"),
      todo("pending", "d")
    ]);
    expect(summary.total).toBe(4);
    expect(summary.completed).toBe(1);
    expect(summary.cancelled).toBe(1);
    expect(summary.ratio).toBe(0.5);
    expect(summary.allDone).toBe(false);
  });

  it("优先展示进行中的活动项", () => {
    const summary = summarizeTodos([
      todo("pending", "later"),
      todo("in_progress", "now")
    ]);
    expect(summary.activeText).toBe("now");
  });

  it("没有进行中时回退到待处理项", () => {
    const summary = summarizeTodos([todo("completed", "done"), todo("pending", "next")]);
    expect(summary.activeText).toBe("next");
  });

  it("全部完成时标记 allDone", () => {
    const summary = summarizeTodos([todo("completed", "a"), todo("cancelled", "b")]);
    expect(summary.allDone).toBe(true);
    expect(summary.ratio).toBe(1);
    expect(summary.activeText).toBeNull();
  });
});
