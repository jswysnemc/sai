import { describe, expect, it } from "vitest";
import { focusIdFromEvent, requestSubagentFocus, takePendingSubagentFocus } from "./subagent-focus";

/** 构造带载荷的事件替身，避免测试依赖浏览器环境。 */
function eventWithDetail(detail: unknown): Event {
  return { detail } as unknown as Event;
}

describe("subagent focus", () => {
  it("面板挂载前的请求可以被首次渲染认领", () => {
    requestSubagentFocus("sub_1");

    expect(takePendingSubagentFocus()).toBe("sub_1");
    // 认领后清空，避免下次打开面板时重复跳转
    expect(takePendingSubagentFocus()).toBeNull();
  });

  it("后一次请求覆盖前一次", () => {
    requestSubagentFocus("sub_1");
    requestSubagentFocus("sub_2");

    expect(takePendingSubagentFocus()).toBe("sub_2");
  });

  it("载荷缺少标识时不产生聚焦", () => {
    expect(focusIdFromEvent(eventWithDetail({ id: "sub_3" }))).toBe("sub_3");
    expect(focusIdFromEvent(eventWithDetail({}))).toBeNull();
    expect(focusIdFromEvent(eventWithDetail(undefined))).toBeNull();
    expect(focusIdFromEvent(eventWithDetail({ id: "" }))).toBeNull();
  });
});
