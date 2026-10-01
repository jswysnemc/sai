import type { BrowserKeyMessage, BrowserMouseMessage } from "./browser-protocol";

/** CDP 修饰键位：Alt=1、Ctrl=2、Meta=4、Shift=8。 */
type ModifierSource = { altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean };

/** 视口在页面上的位置与页面 CSS 尺寸，用于坐标换算。 */
export type ViewportGeometry = {
  left: number;
  top: number;
  width: number;
  height: number;
  pageWidth: number;
  pageHeight: number;
};

/** 不转发给远端页面、保留给面板自身的按键。 */
const LOCAL_ONLY_KEYS = new Set(["F5", "F12"]);

/**
 * 把 DOM 事件的修饰键转换为 CDP 修饰键位。
 *
 * @param event 鼠标或键盘事件
 * @returns 修饰键位
 */
export function modifierBits(event: ModifierSource): number {
  return (event.altKey ? 1 : 0) | (event.ctrlKey ? 2 : 0) | (event.metaKey ? 4 : 0) | (event.shiftKey ? 8 : 0);
}

/**
 * 把 DOM 鼠标按键编号转换为 CDP 按键名。
 *
 * @param button MouseEvent.button
 * @returns CDP 按键名
 */
export function mouseButtonName(button: number): "left" | "middle" | "right" | "none" {
  if (button === 0) return "left";
  if (button === 1) return "middle";
  if (button === 2) return "right";
  return "none";
}

/**
 * 把视口内的客户端坐标换算为页面 CSS 像素。
 *
 * 画面按比例缩放显示，换算后裁剪到页面范围内。
 *
 * @param clientX 客户端横坐标
 * @param clientY 客户端纵坐标
 * @param geometry 视口几何信息
 * @returns 页面坐标
 */
export function toPagePoint(clientX: number, clientY: number, geometry: ViewportGeometry): { x: number; y: number } {
  const scaleX = geometry.width > 0 ? geometry.pageWidth / geometry.width : 1;
  const scaleY = geometry.height > 0 ? geometry.pageHeight / geometry.height : 1;
  const x = Math.min(Math.max((clientX - geometry.left) * scaleX, 0), geometry.pageWidth);
  const y = Math.min(Math.max((clientY - geometry.top) * scaleY, 0), geometry.pageHeight);
  return { x: Math.round(x * 10) / 10, y: Math.round(y * 10) / 10 };
}

/**
 * 构造鼠标消息。
 *
 * @param kind 事件类型
 * @param event 原始鼠标事件
 * @param geometry 视口几何信息
 * @returns 鼠标消息
 */
export function mouseMessage(
  kind: BrowserMouseMessage["kind"],
  event: MouseEvent,
  geometry: ViewportGeometry
): BrowserMouseMessage {
  const point = toPagePoint(event.clientX, event.clientY, geometry);
  const message: BrowserMouseMessage = { type: "mouse", kind, ...point, modifiers: modifierBits(event) };
  if (kind === "down" || kind === "up") {
    message.button = mouseButtonName(event.button);
    message.click_count = Math.max(1, event.detail || 1);
  } else if (kind === "move") {
    // 拖拽选择时需要带上按住的按键
    message.button = event.buttons & 1 ? "left" : event.buttons & 2 ? "right" : "none";
  }
  if (kind === "wheel" && event instanceof WheelEvent) {
    // deltaMode 为行或页时换算为像素
    const unit = event.deltaMode === 1 ? 40 : event.deltaMode === 2 ? geometry.pageHeight : 1;
    message.delta_x = event.deltaX * unit;
    message.delta_y = event.deltaY * unit;
  }
  return message;
}

/**
 * 构造键盘消息；面板保留键与输入法组合过程中的按键返回 null。
 *
 * @param kind 按下或抬起
 * @param event 原始键盘事件
 * @returns 键盘消息
 */
export function keyMessage(kind: "down" | "up", event: KeyboardEvent): BrowserKeyMessage | null {
  if (event.isComposing || event.key === "Process" || event.key === "Unidentified") return null;
  if (LOCAL_ONLY_KEYS.has(event.key)) return null;
  const modifiers = modifierBits(event);
  const message: BrowserKeyMessage = {
    type: "key",
    kind,
    key: event.key,
    code: event.code,
    key_code: legacyKeyCode(event),
    modifiers
  };
  // 只有不带 Ctrl/Meta 的可打印字符与回车会产生输入
  const printable = event.key.length === 1 || event.key === "Enter";
  if (kind === "down" && printable && (modifiers & 6) === 0) {
    message.text = event.key === "Enter" ? "\r" : event.key;
  }
  return message;
}

/**
 * 判断按键是否应交给浏览器原生处理（粘贴需要触发 paste 事件读取剪贴板）。
 *
 * @param event 键盘事件
 * @returns 应放行原生行为时为 true
 */
export function isNativePasteShortcut(event: KeyboardEvent): boolean {
  return (event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "v";
}

/**
 * 读取键盘事件的 Windows 虚拟键码。
 *
 * @param event 键盘事件
 * @returns 虚拟键码
 */
function legacyKeyCode(event: KeyboardEvent): number {
  // keyCode 已废弃但仍是各浏览器一致的虚拟键码来源
  const code = (event as KeyboardEvent & { keyCode?: number }).keyCode ?? 0;
  if (code) return code;
  if (event.key.length === 1) return event.key.toUpperCase().charCodeAt(0);
  return 0;
}
