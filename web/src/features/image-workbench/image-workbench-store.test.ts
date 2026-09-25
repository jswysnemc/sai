import { describe, expect, it } from "vitest";
import { appendImageTurn, createImageSession, imageFollowUpPrompt, removeImageSession, removeImageSessions, titleFromPrompt, type ImageWorkbenchStore, type ImageWorkbenchTurn } from "./image-workbench-store";

const turn: ImageWorkbenchTurn = {
  id: "turn-1",
  prompt: "一只站在窗边的猫",
  status: "loading",
  aspectRatio: "1:1",
  resolution: "1024x1024",
  model: "image-1",
  createdAt: "2026-09-23T00:00:00.000Z"
};

/**
 * 造一个只有空会话的存储。
 *
 * @returns 测试存储
 */
function store(): ImageWorkbenchStore {
  return {
    activeId: "image-a",
    sessions: [{ id: "image-a", title: "新的图片", createdAt: turn.createdAt, updatedAt: turn.createdAt, turns: [] }]
  };
}

describe("image workbench sessions", () => {
  it("用首条提示词作为会话标题，并保留后续轮次", () => {
    const next = appendImageTurn(store(), turn);
    expect(next.sessions[0].title).toBe("一只站在窗边的猫");
    expect(next.sessions[0].turns).toHaveLength(1);
    expect(titleFromPrompt("  多   空格  ")).toBe("多 空格");
  });

  it("删除最后一条会话后补一个空会话", () => {
    const next = removeImageSession(store(), "image-a");
    expect(next.sessions).toHaveLength(1);
    expect(next.sessions[0].id).not.toBe("image-a");
    expect(next.activeId).toBe(next.sessions[0].id);
  });

  it("一次删除多条会话，并在当前会话被删掉后改选剩下的", () => {
    const next = createImageSession(store(), "2026-09-23T01:00:00.000Z");
    const kept = next.sessions[1].id;
    const removed = removeImageSessions(next, [next.sessions[0].id]);
    expect(removed.sessions.map((session) => session.id)).toEqual([kept]);
    expect(removed.activeId).toBe(kept);
  });

  it("新建会话后成为当前会话", () => {
    const next = createImageSession(store(), "2026-09-23T01:00:00.000Z");
    expect(next.sessions[0].id).toBe(next.activeId);
    expect(next.sessions).toHaveLength(2);
  });

  it("后续轮次带上先前提示词", () => {
    expect(imageFollowUpPrompt([], "一只猫")).toBe("一只猫");
    expect(imageFollowUpPrompt(["一只猫", "  "], "改成夜晚")).toContain("1. 一只猫");
    expect(imageFollowUpPrompt(["一只猫"], "改成夜晚")).toContain("改成夜晚");
  });
});
