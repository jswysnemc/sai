import { describe, expect, it } from "vitest";
import { isNativePasteShortcut, keyMessage, modifierBits, mouseButtonName, toPagePoint } from "./browser-input";
import { parseBrowserServerMessage } from "./browser-protocol";

/**
 * 构造最小键盘事件替身。
 *
 * @param init 键盘事件字段
 * @returns 可传给 keyMessage 的事件
 */
function keyEvent(init: Partial<KeyboardEvent> & { key: string }): KeyboardEvent {
  return {
    code: "",
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    isComposing: false,
    keyCode: 0,
    ...init
  } as KeyboardEvent;
}

describe("browser input mapping", () => {
  it("把显示坐标按比例换算为页面坐标并裁剪到页面内", () => {
    const geometry = { left: 100, top: 50, width: 640, height: 400, pageWidth: 1280, pageHeight: 800 };
    expect(toPagePoint(420, 250, geometry)).toEqual({ x: 640, y: 400 });
    expect(toPagePoint(0, 0, geometry)).toEqual({ x: 0, y: 0 });
    expect(toPagePoint(2000, 2000, geometry)).toEqual({ x: 1280, y: 800 });
  });

  it("修饰键与鼠标按键对应 CDP 编码", () => {
    expect(modifierBits({ altKey: true, ctrlKey: true, metaKey: false, shiftKey: true })).toBe(11);
    expect(mouseButtonName(0)).toBe("left");
    expect(mouseButtonName(2)).toBe("right");
    expect(mouseButtonName(4)).toBe("none");
  });

  it("可打印字符带输入文本，Ctrl 组合键与功能键不带", () => {
    expect(keyMessage("down", keyEvent({ key: "a", code: "KeyA", keyCode: 65 }))).toMatchObject({ text: "a", key_code: 65 });
    expect(keyMessage("down", keyEvent({ key: "Enter", code: "Enter", keyCode: 13 }))?.text).toBe("\r");
    expect(keyMessage("down", keyEvent({ key: "a", ctrlKey: true }))?.text).toBeUndefined();
    expect(keyMessage("down", keyEvent({ key: "ArrowDown", keyCode: 40 }))?.text).toBeUndefined();
    expect(keyMessage("up", keyEvent({ key: "a" }))?.text).toBeUndefined();
  });

  it("输入法组合过程与面板保留键不转发，粘贴快捷键放行原生处理", () => {
    expect(keyMessage("down", keyEvent({ key: "n", isComposing: true }))).toBeNull();
    expect(keyMessage("down", keyEvent({ key: "Process" }))).toBeNull();
    expect(keyMessage("down", keyEvent({ key: "F12" }))).toBeNull();
    expect(isNativePasteShortcut(keyEvent({ key: "v", ctrlKey: true }))).toBe(true);
    expect(isNativePasteShortcut(keyEvent({ key: "v" }))).toBe(false);
  });

  it("只接受结构完整的服务端消息", () => {
    expect(parseBrowserServerMessage('{"type":"activity","message":"Clicking e3"}')).toEqual({ type: "activity", message: "Clicking e3" });
    expect(parseBrowserServerMessage('{"type":"state","state":{"url":"https://a.b"}}')?.type).toBe("state");
    expect(parseBrowserServerMessage('{"type":"error"}')).toBeNull();
    expect(parseBrowserServerMessage("not json")).toBeNull();
  });
});
