import { afterEach, describe, expect, it, vi } from "vitest";
import { handleWorkbenchShortcut, resolveWorkbenchShortcut } from "./workbench-shortcuts";

afterEach(() => vi.unstubAllGlobals());

/**
 * 组合键盘状态，验证不同平台和输入环境下的工作台动作。
 * @param overrides 当前用例需要覆盖的按键状态
 * @returns 快捷键解析器所需的事件字段
 */
function shortcut(overrides: Partial<Parameters<typeof resolveWorkbenchShortcut>[0]>) {
  return { key: "k", ctrlKey: true, metaKey: false, altKey: false, shiftKey: false, isComposing: false, defaultPrevented: false, repeat: false, ...overrides };
}

describe("工作台快捷键", () => {
  it.each([
    ["k", false, "search"],
    ["p", true, "search"],
    ["o", true, "new-session"],
    ["b", false, "toggle-sidebar"],
    ["j", false, "toggle-terminal"],
    ["e", true, "open-files"],
    ["g", true, "open-changes"],
    ["i", false, "focus-composer"]
  ] as const)("Ctrl 和 Command 使用同一动作：%s", (key, shiftKey, command) => {
    expect(resolveWorkbenchShortcut(shortcut({ key, shiftKey }))).toBe(command);
    expect(resolveWorkbenchShortcut(shortcut({ key: key.toUpperCase(), shiftKey, ctrlKey: false, metaKey: true }))).toBe(command);
  });

  it.each([
    { isComposing: true },
    { repeat: true },
    { defaultPrevented: true },
    { altKey: true },
    { ctrlKey: false },
    { shiftKey: true },
    { key: "n" },
    { key: "o" }
  ])("保留输入法、已处理事件和未注册组合键：%j", (overrides) => {
    expect(resolveWorkbenchShortcut(shortcut(overrides))).toBeNull();
  });

  it("长按已注册组合键时阻止终端输入，但不重复切换面板", () => {
    const dispatchEvent = vi.fn();
    vi.stubGlobal("document", { querySelector: () => null });
    vi.stubGlobal("window", { dispatchEvent });
    const preventDefault = vi.fn();
    const event = { ...shortcut({ key: "j", repeat: true }), preventDefault } as unknown as KeyboardEvent;
    expect(handleWorkbenchShortcut(event)).toBe(true);
    expect(preventDefault).toHaveBeenCalledOnce();
    expect(dispatchEvent).not.toHaveBeenCalled();
  });

  it("弹层打开后保留其键盘操作，不触发后台布局切换", () => {
    const dispatchEvent = vi.fn();
    vi.stubGlobal("document", { querySelector: () => ({ role: "dialog" }) });
    vi.stubGlobal("window", { dispatchEvent });
    const preventDefault = vi.fn();
    const event = { ...shortcut({ key: "j" }), preventDefault } as unknown as KeyboardEvent;
    expect(handleWorkbenchShortcut(event)).toBe(false);
    expect(preventDefault).not.toHaveBeenCalled();
    expect(dispatchEvent).not.toHaveBeenCalled();
  });
});
