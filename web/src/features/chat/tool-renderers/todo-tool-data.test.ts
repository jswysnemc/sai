import { describe, expect, it } from "vitest";
import { parseTodoTool, todoToolHeadline } from "./todo-tool-data";

describe("parseTodoTool", () => {
  it("解析新增任务的参数与结果", () => {
    const summary = parseTodoTool(
      JSON.stringify({ action: "add", text: "写周报" }),
      JSON.stringify({ ok: true, changed: [{ id: "1", text: "写周报", status: "pending" }], items: [{ id: "1", text: "写周报", status: "pending" }] })
    );
    expect(summary.action).toBe("add");
    expect(summary.text).toBe("写周报");
    expect(summary.changedIds).toEqual(["1"]);
    expect(todoToolHeadline(summary)).toBe("创建任务：写周报");
  });

  it("解析批量创建", () => {
    const summary = parseTodoTool(
      JSON.stringify({ action: "add", texts: ["读代码", "改实现", "补测试"] }),
      JSON.stringify({ ok: true, changed: [{ id: "1" }, { id: "2" }, { id: "3" }], items: [{ id: "1" }, { id: "2" }, { id: "3" }] })
    );
    expect(summary.texts).toHaveLength(3);
    expect(summary.itemCount).toBe(3);
    expect(todoToolHeadline(summary)).toBe("创建 3 个任务");
  });

  it("解析状态更新", () => {
    const summary = parseTodoTool(
      JSON.stringify({ action: "update", index: 1, status: "completed" }),
      JSON.stringify({ ok: true, changed: [{ id: "1", text: "写周报", status: "completed" }], items: [{ id: "1", text: "写周报", status: "completed" }] })
    );
    expect(summary.action).toBe("update");
    expect(summary.status).toBe("completed");
    expect(summary.text).toBe("写周报");
    expect(todoToolHeadline(summary)).toBe("更新任务：写周报");
  });

  it("兼容旧格式的单条 item 输出", () => {
    const summary = parseTodoTool(
      JSON.stringify({ action: "remove", id: "1" }),
      JSON.stringify({ ok: true, item: { id: "1", text: "写周报", status: "pending" } })
    );
    expect(summary.text).toBe("写周报");
    expect(summary.changedIds).toEqual(["1"]);
    expect(todoToolHeadline(summary)).toBe("删除任务：写周报");
  });

  it("解析清单查看的条目数", () => {
    const summary = parseTodoTool(
      JSON.stringify({ action: "list" }),
      JSON.stringify({ ok: true, items: [{ id: "1" }, { id: "2" }, { id: "3" }] })
    );
    expect(summary.action).toBe("list");
    expect(summary.itemCount).toBe(3);
    expect(todoToolHeadline(summary)).toBe("查看计划清单（3 项）");
  });

  it("容错非法 JSON", () => {
    const summary = parseTodoTool("not json", "also not json");
    expect(summary.action).toBe("unknown");
    expect(summary.itemCount).toBeNull();
    expect(todoToolHeadline(summary)).toBe("更新计划清单");
  });
});
