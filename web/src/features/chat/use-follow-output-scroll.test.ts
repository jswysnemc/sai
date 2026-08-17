import { describe, expect, it } from "vitest";
import { canProgrammaticFollow, isNearOutputBottom, resolveFollowOutputState, scrollOutputToBottom, snapScrollTopToLine } from "./use-follow-output-scroll";

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

  it("程序滚动和内容增长不会在用户上滚后恢复跟随", () => {
    expect(resolveFollowOutputState(
      { following: false, showJump: true },
      { scrollTop: 400, scrollHeight: 1400, clientHeight: 300 },
      false
    )).toEqual({ following: false, showJump: true });
  });

  it("将持续增长的输出区域滚动到最新位置", () => {
    const element = { scrollTop: 120, scrollHeight: 960 };

    scrollOutputToBottom(element);

    expect(element.scrollTop).toBe(960);
  });

  it("用户滚轮窗口内不让程序贴底抢走视口", () => {
    expect(canProgrammaticFollow(true, 1000, 999)).toBe(false);
    expect(canProgrammaticFollow(true, 1000, 1001)).toBe(true);
    expect(canProgrammaticFollow(false, 1000, 2000)).toBe(false);
  });

  it("把贴底滚动对齐到整行，避免顶边裁出半行", () => {
    expect(snapScrollTopToLine(54, 20, 4, 54)).toBe(44);
    expect(snapScrollTopToLine(44, 20, 4, 54)).toBe(44);
    expect(snapScrollTopToLine(0, 20, 4, 54)).toBe(0);
    expect(snapScrollTopToLine(80, 20, 4, 54)).toBe(44);
    expect(snapScrollTopToLine(54, 0, 4, 54)).toBe(54);
  });
});
