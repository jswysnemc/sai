import { describe, expect, it } from "vitest";
import {
  enqueueFrame,
  framesToPlay,
  MAX_QUEUED_FRAMES,
  type SummaryFrame
} from "./summary-frame-queue";

/**
 * 构造测试用摘要帧。
 *
 * @param key 帧标识
 * @returns 摘要帧
 */
function frame(key: string): SummaryFrame {
  return { key, primaryText: key, secondaryText: "" };
}

describe("enqueueFrame", () => {
  it("takes the first frame directly", () => {
    expect(enqueueFrame([], frame("a"))).toEqual([frame("a")]);
  });

  it("ignores a frame whose key is already queued", () => {
    // 流式更新会反复推送同一状态，不去重会让队列被同一内容占满
    const queued = [frame("a"), frame("b")];
    expect(enqueueFrame(queued, frame("b"))).toEqual(queued);
  });

  it("keeps the oldest and newest frame when the queue overflows", () => {
    const queued = [frame("a"), frame("b")];

    const next = enqueueFrame(queued, frame("c"));

    expect(next).toHaveLength(MAX_QUEUED_FRAMES);
    expect(next[0].key).toBe("a");
    expect(next[1].key).toBe("c");
  });
});

describe("framesToPlay", () => {
  it("plays every frame while keeping up", () => {
    const queued = [frame("a"), frame("b")];
    expect(framesToPlay(queued, 10)).toEqual(queued);
  });

  it("skips to the last frame when it has fallen behind", () => {
    // 逐帧播完会让显示状态明显滞后于真实状态
    const queued = [frame("a"), frame("b")];
    expect(framesToPlay(queued, 5_000)).toEqual([frame("b")]);
  });

  it("keeps a single queued frame even when behind", () => {
    expect(framesToPlay([frame("a")], 5_000)).toEqual([frame("a")]);
  });
});
