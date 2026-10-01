/**
 * 内置浏览器面板与服务端之间的 WebSocket 协议。
 *
 * 文本帧是 JSON 控制或状态消息，二进制帧是一张 JPEG 页面画面。
 * 字段命名与 Rust 端 `src/web/browser/protocol.rs` 保持一致（snake_case）。
 */

/** 一个标签页。 */
export type BrowserTab = {
  id: string;
  title: string;
  url: string;
  active: boolean;
};

/** 地址栏、标签条与视口状态。 */
export type BrowserState = {
  tabs: BrowserTab[];
  url: string;
  title: string;
  loading: boolean;
  can_go_back: boolean;
  can_go_forward: boolean;
  width: number;
  height: number;
};

/** 服务端推送的文本消息。 */
export type BrowserServerMessage =
  | { type: "state"; state: BrowserState }
  | { type: "activity"; message: string }
  | { type: "error"; message: string };

/** 鼠标事件，坐标为页面 CSS 像素。 */
export type BrowserMouseMessage = {
  type: "mouse";
  kind: "move" | "down" | "up" | "wheel";
  x: number;
  y: number;
  button?: "left" | "middle" | "right" | "none";
  click_count?: number;
  delta_x?: number;
  delta_y?: number;
  modifiers: number;
};

/** 键盘事件。 */
export type BrowserKeyMessage = {
  type: "key";
  kind: "down" | "up";
  key: string;
  code: string;
  key_code: number;
  text?: string;
  modifiers: number;
};

/** 面板发给服务端的控制消息。 */
export type BrowserClientMessage =
  | { type: "navigate"; url: string }
  | { type: "back" }
  | { type: "forward" }
  | { type: "reload" }
  | { type: "stop" }
  | BrowserMouseMessage
  | BrowserKeyMessage
  | { type: "insert_text"; text: string }
  | { type: "resize"; width: number; height: number }
  | { type: "new_tab" }
  | { type: "switch_tab"; id: string }
  | { type: "close_tab"; id: string };

/**
 * 解析服务端文本消息，结构不符时返回 null。
 *
 * @param raw WebSocket 文本帧
 * @returns 已识别的消息
 */
export function parseBrowserServerMessage(raw: string): BrowserServerMessage | null {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!value || typeof value !== "object") return null;
  const message = value as Record<string, unknown>;
  if (message.type === "state" && message.state && typeof message.state === "object") {
    return { type: "state", state: message.state as BrowserState };
  }
  if ((message.type === "activity" || message.type === "error") && typeof message.message === "string") {
    return { type: message.type, message: message.message };
  }
  return null;
}

/** 浏览器面板 WebSocket 地址。 */
export function browserSocketUrl(): string {
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${location.host}/api/browser/socket`;
}
