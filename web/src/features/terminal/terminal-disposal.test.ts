import { afterEach, describe, expect, it, vi } from "vitest";
import { disposeTerminalView } from "./terminal-disposal";

afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
});

describe("终端实例释放", () => {
  it("立即移除旧视图，等待已经排队的初始化完成后释放资源", () => {
    vi.useFakeTimers();
    let disposed = false;
    const events: string[] = [];
    const terminal = {
      element: { remove: vi.fn() },
      dispose: vi.fn(() => { disposed = true; events.push("disposed"); })
    };
    // 1. 模拟 Xterm 在 open 内排队的视口初始化任务
    setTimeout(() => {
      if (disposed) throw new Error("dimensions accessed after disposal");
      events.push("initialized");
    }, 0);
    // 2. 模拟组件挂载后立即清理，视图必须先脱离原容器
    disposeTerminalView(terminal);
    expect(terminal.element.remove).toHaveBeenCalledOnce();
    expect(() => vi.runAllTimers()).not.toThrow();
    expect(events).toEqual(["initialized", "disposed"]);
    expect(terminal.dispose).toHaveBeenCalledOnce();
  });

  it("未创建视图的实例仍然能释放资源", () => {
    vi.useFakeTimers();
    const dispose = vi.fn();
    disposeTerminalView({ dispose });
    vi.runAllTimers();
    expect(dispose).toHaveBeenCalledOnce();
  });
});
