import { describe, expect, it } from "vitest";
import { nextTodoStatus, todoStatusLabel } from "./todo-status";

describe("todo status", () => {
  it("按待处理、进行中、已完成、已取消的顺序循环", () => {
    expect(nextTodoStatus("pending")).toBe("in_progress");
    expect(nextTodoStatus("in_progress")).toBe("completed");
    expect(nextTodoStatus("completed")).toBe("cancelled");
    expect(nextTodoStatus("cancelled")).toBe("pending");
  });

  it("返回用于界面展示的中文状态名称", () => {
    expect(todoStatusLabel("pending")).toBe("待处理");
    expect(todoStatusLabel("in_progress")).toBe("进行中");
    expect(todoStatusLabel("completed")).toBe("已完成");
    expect(todoStatusLabel("cancelled")).toBe("已取消");
  });
});
