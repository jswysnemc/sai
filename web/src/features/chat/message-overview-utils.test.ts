import { describe, expect, it } from "vitest";
import type { SessionTimelineTurn } from "../../api/contracts";
import { initialRunState } from "./run-event-reducer";
import {
  createLiveOverviewItem,
  createOverviewSummary,
  createTimelineOverviewItems,
  evenlySpacedOverviewPosition
} from "./message-overview-utils";

/**
 * 创建概览测试使用的历史轮次。
 *
 * @param patch 需要覆盖的轮次字段
 * @returns 完整历史轮次
 */
function timelineTurn(patch: Partial<SessionTimelineTurn> = {}): SessionTimelineTurn {
  return {
    turn_id: "turn-1",
    seq: 1,
    status: "completed",
    automatic: false,
    user: { timestamp: "2026-07-11T00:00:00Z", content: "修复 `src/chat-page.tsx` 的布局", reasoning: null },
    assistant: {
      timestamp: "2026-07-11T00:01:00Z",
      content: "已调整 **编辑器布局**，并修改 web/src/features/chat/chat-page.css。",
      reasoning: null
    },
    tools: [],
    ...patch
  };
}

describe("message overview utils", () => {
  it("清理 Markdown、压缩空白并按字符数截断", () => {
    const source = "# 标题\n\n- [链接](https://example.com)  **正文**\n- `src/chat.tsx`";

    expect(createOverviewSummary(source)).toBe("标题 链接 正文 src/chat.tsx");
    expect(createOverviewSummary("一二三四五", 4)).toBe("一二三…");
    expect(createOverviewSummary("一二三", 1)).toBe("…");
  });

  it("从历史轮次生成标题、摘要、状态和去重文件标签", () => {
    const turn = timelineTurn({
      tools: [{
        id: "tool-1",
        name: "edit_file",
        arguments: JSON.stringify({ path: "src/tools/edit_file.rs", files: [{ path: "src/tools/run.rs" }] }),
        status: "completed",
        output: "Updated src/tools/edit_file.rs and web/src/chat-page.tsx",
        created_at: "2026-07-11T00:00:30Z"
      }]
    });

    const [item] = createTimelineOverviewItems([turn]);

    expect(item).toMatchObject({
      id: "turn-turn-1",
      category: "history",
      label: "已完成",
      title: "修复 src/chat-page.tsx 的布局",
      summary: "已调整 编辑器布局，并修改 web/src/features/chat/chat-page.css。",
      status: "completed"
    });
    expect(item.tags).toEqual(["chat-page.tsx", "chat-page.css", "edit_file.rs", "run.rs"]);
    expect(item.hiddenTagCount).toBe(0);
  });

  it("空助手内容回退轮次状态并限制标签数量", () => {
    const turn = timelineTurn({
      status: "interrupted",
      assistant: { timestamp: "", content: "", reasoning: null },
      user: { timestamp: "", content: "检查 a.ts b.ts c.ts d.ts e.ts", reasoning: null }
    });

    const [item] = createTimelineOverviewItems([turn]);

    expect(item.summary).toBe("已中断");
    expect(item.tags).toEqual(["a.ts", "b.ts", "c.ts", "d.ts"]);
    expect(item.hiddenTagCount).toBe(1);
  });

  it("为实时状态生成稳定 ID并优先使用正文摘要", () => {
    const item = createLiveOverviewItem({
      ...initialRunState,
      runId: "run-7",
      userInput: "检查构建",
      content: "正在修改 src/main.ts",
      status: "working"
    });

    expect(item).toMatchObject({
      id: "live-run-7",
      category: "live",
      label: "工作中",
      title: "检查构建",
      summary: "正在修改 src/main.ts",
      status: "running"
    });
    expect(createLiveOverviewItem(initialRunState)).toBeNull();
  });

  it("实时正文为空时回退当前状态", () => {
    const [item] = createTimelineOverviewItems([], {
      ...initialRunState,
      runId: "run-8",
      userInput: "等待模型",
      status: "waiting_response"
    });

    expect(item.summary).toBe("等待响应");
  });

  it("按项目顺序生成均匀的概览标识位置", () => {
    expect(evenlySpacedOverviewPosition(0, 4, 100)).toBe(29);
    expect(evenlySpacedOverviewPosition(1, 4, 100)).toBe(43);
    expect(evenlySpacedOverviewPosition(2, 4, 100)).toBe(57);
    expect(evenlySpacedOverviewPosition(3, 4, 100)).toBe(71);
    expect(evenlySpacedOverviewPosition(0, 1, 100)).toBe(50);
    expect(evenlySpacedOverviewPosition(2, 3, 20)).toBe(20);
  });
});
