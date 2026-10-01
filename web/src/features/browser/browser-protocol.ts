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

/** 页面弹出的 alert、confirm、prompt 或离开页面确认。 */
export type BrowserDialog = {
  kind: "alert" | "confirm" | "prompt" | "beforeunload" | string;
  message: string;
  default_prompt: string;
  url: string;
};

/** 原生下拉框的一个选项。 */
export type BrowserSelectOption = { label: string; disabled: boolean; group: string };

/** 面板需要绘制的原生下拉菜单，坐标为页面 CSS 像素。 */
export type BrowserSelectPopup = {
  options: BrowserSelectOption[];
  selected: number;
  x: number;
  y: number;
  width: number;
  height: number;
};

/** 页面请求选择文件。 */
export type BrowserFileChooser = { multiple: boolean; accept: string };

/** 一次页面下载。 */
export type BrowserDownload = {
  guid: string;
  file_name: string;
  url: string;
  state: "inProgress" | "completed" | "canceled" | string;
  received: number;
  total: number;
};

/** 元素选择得到的网页元素信息。 */
export type BrowserPickedElement = {
  pageUrl: string;
  pageTitle: string;
  tagName: string;
  role?: string;
  accessibleName?: string;
  selector?: string;
  xpath?: string;
  text?: string;
  nearbyText?: string;
  htmlExcerpt?: string;
  attributes?: Record<string, string>;
  rect?: { x: number; y: number; width: number; height: number };
  style?: Record<string, string>;
};

/** 服务端推送的文本消息。 */
export type BrowserServerMessage =
  | { type: "state"; state: BrowserState }
  | { type: "activity"; message: string }
  | { type: "error"; message: string }
  | { type: "info"; devtools_url: string | null; persistent_profile: boolean }
  | { type: "dialog"; dialog: BrowserDialog | null }
  | { type: "select_popup"; popup: BrowserSelectPopup }
  | { type: "file_chooser"; chooser: BrowserFileChooser | null }
  | { type: "download"; download: BrowserDownload }
  | { type: "clipboard"; text: string }
  | { type: "picked"; element: BrowserPickedElement | null };

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
  | { type: "resize"; width: number; height: number; scale: number }
  | { type: "new_tab" }
  | { type: "switch_tab"; id: string }
  | { type: "close_tab"; id: string }
  | { type: "dialog_reply"; accept: boolean; prompt_text?: string }
  | { type: "select_reply"; index: number }
  | { type: "file_chooser_reply"; uploads: string[] }
  | { type: "pick_start"; labels: [string, string, string] }
  | { type: "pick_cancel" };

/** 带对象载荷的服务端消息类型与载荷字段。 */
const PAYLOAD_FIELDS: Record<string, string> = {
  dialog: "dialog",
  select_popup: "popup",
  file_chooser: "chooser",
  download: "download",
  picked: "element"
};

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
  if (message.type === "info") {
    return {
      type: "info",
      devtools_url: typeof message.devtools_url === "string" ? message.devtools_url : null,
      persistent_profile: message.persistent_profile === true
    };
  }
  if (message.type === "clipboard" && typeof message.text === "string") {
    return { type: "clipboard", text: message.text };
  }
  // 对话框、文件选择与选择结果允许为 null，表示已关闭或已取消
  const field = typeof message.type === "string" ? PAYLOAD_FIELDS[message.type] : undefined;
  if (field && field in message) {
    const payload = message[field];
    const nullable = message.type === "dialog" || message.type === "file_chooser" || message.type === "picked";
    if (payload === null ? nullable : typeof payload === "object") {
      return { type: message.type, [field]: payload } as BrowserServerMessage;
    }
  }
  return null;
}

/** 浏览器面板 WebSocket 地址。 */
export function browserSocketUrl(): string {
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${location.host}/api/browser/socket`;
}

/**
 * 判断当前页面是否在本机访问；调试工具地址只在本机可用。
 *
 * @returns 本机访问时为 true
 */
export function isLocalWorkbench(): boolean {
  return ["localhost", "127.0.0.1", "[::1]", "::1"].includes(location.hostname);
}
