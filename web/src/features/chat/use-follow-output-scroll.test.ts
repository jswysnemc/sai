import { describe, expect, it } from "vitest";
import { isNearOutputBottom, resolveFollowOutputState, scrollOutputToBottom } from "./use-follow-output-scroll";

describe("follow output scroll", () => {
  it("将底部容差内的位置视为正在跟随", () => {
    expect(isNearOutputBottom({ scrollTop: 821, scrollHeight: 1200, clientHeight: 300 })).toBe(true);
    expect(isNearOutputBottom({ scrollTop: 800, scrollHeight: 1200, clientHeight: 300 })).toBe(false);
  });

  it("用户主动向上滚动后暂停自动跟随", () => {
    expect(resolveFollowOutputState(
      { following: true, showJump: false },
      { scrollTop: 400, scrollHeight: 1200, clientHeight: 300 },
      true
    )).toEqual({ following: false, showJump: true });
  });

  it("程序滚动和内容增长不会误判为用户接管", () => {
    expect(resolveFollowOutputState(
      { following: true, showJump: false },
      { scrollTop: 790, scrollHeight: 1200, clientHeight: 300 },
      false
    )).toEqual({ following: true, showJump: false });
  });

  it("用户主动回到底部后恢复思考和正文跟随", () => {
    expect(resolveFollowOutputState(
      { following: false, showJump: true },
      { scrollTop: 900, scrollHeight: 1200, clientHeight: 300 },
      true
    )).toEqual({ following: true, showJump: false });
  });

  it("将持续增长的输出区域滚动到最新位置", () => {
    const element = { scrollTop: 120, scrollHeight: 960 };

    scrollOutputToBottom(element);

    expect(element.scrollTop).toBe(960);
  });
});
