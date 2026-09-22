import type { ImageAspectRatio, ImageResolution } from "../chat/image-generation/image-generation-options";

/** 随一轮请求附上的参考图。 */
export type ImageWorkbenchAttachment = {
  name: string;
  dataUrl: string;
};

/** 一轮生图请求及其结果。 */
export type ImageWorkbenchTurn = {
  id: string;
  prompt: string;
  status: "loading" | "success" | "error";
  output?: string;
  error?: string;
  aspectRatio: ImageAspectRatio;
  resolution: ImageResolution;
  model: string;
  createdAt: string;
  attachments?: ImageWorkbenchAttachment[];
};

/** 一次独立的生图会话。 */
export type ImageWorkbenchSession = {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  turns: ImageWorkbenchTurn[];
};

export type ImageWorkbenchStore = {
  activeId: string;
  sessions: ImageWorkbenchSession[];
};

const STORAGE_KEY = "sai.image-workbench.sessions";

/**
 * 读取本地生图会话。损坏或为空时返回一个新会话。
 *
 * @returns 会话存储
 */
export function loadImageWorkbenchStore(): ImageWorkbenchStore {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return createStore();
    const parsed = JSON.parse(raw) as ImageWorkbenchStore;
    if (!Array.isArray(parsed.sessions) || parsed.sessions.length === 0) return createStore();
    const activeId = parsed.sessions.some((session) => session.id === parsed.activeId)
      ? parsed.activeId
      : parsed.sessions[0].id;
    return { activeId, sessions: parsed.sessions.map(normalizeSession) };
  } catch {
    return createStore();
  }
}

/**
 * 写入本地生图会话。
 *
 * @param store 会话存储
 */
export function saveImageWorkbenchStore(store: ImageWorkbenchStore): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
  } catch {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(stripAttachmentData(store)));
  }
}

/**
 * 新建一个空会话并设为当前会话。
 *
 * @param store 当前存储
 * @param now 创建时间
 * @returns 新存储
 */
export function createImageSession(store: ImageWorkbenchStore, now = new Date().toISOString()): ImageWorkbenchStore {
  const session = emptySession(now);
  return { activeId: session.id, sessions: [session, ...store.sessions] };
}

/**
 * 切换当前会话。
 *
 * @param store 当前存储
 * @param sessionId 目标会话
 * @returns 新存储
 */
export function selectImageSession(store: ImageWorkbenchStore, sessionId: string): ImageWorkbenchStore {
  if (!store.sessions.some((session) => session.id === sessionId)) return store;
  return { ...store, activeId: sessionId };
}

/**
 * 删除会话。最后一条删除后补一个空会话。
 *
 * @param store 当前存储
 * @param sessionId 要删除的会话
 * @returns 新存储
 */
export function removeImageSession(store: ImageWorkbenchStore, sessionId: string): ImageWorkbenchStore {
  const sessions = store.sessions.filter((session) => session.id !== sessionId);
  if (sessions.length === 0) return createStore();
  return { activeId: store.activeId === sessionId ? sessions[0].id : store.activeId, sessions };
}

/**
 * 向当前会话追加一轮，并用首条提示词作为标题。
 *
 * @param store 当前存储
 * @param turn 新轮次
 * @returns 新存储
 */
export function appendImageTurn(store: ImageWorkbenchStore, turn: ImageWorkbenchTurn): ImageWorkbenchStore {
  return mapActive(store, (session) => ({
    ...session,
    title: session.turns.length === 0 ? titleFromPrompt(turn.prompt) : session.title,
    updatedAt: turn.createdAt,
    turns: [...session.turns, turn]
  }));
}

/**
 * 更新当前会话中的一轮。
 *
 * @param store 当前存储
 * @param turnId 轮次标识
 * @param patch 要合并的字段
 * @returns 新存储
 */
export function updateImageTurn(store: ImageWorkbenchStore, turnId: string, patch: Partial<ImageWorkbenchTurn>): ImageWorkbenchStore {
  const updatedAt = new Date().toISOString();
  return {
    ...store,
    sessions: store.sessions.map((session) => session.turns.some((turn) => turn.id === turnId)
      ? { ...session, updatedAt, turns: session.turns.map((turn) => turn.id === turnId ? { ...turn, ...patch } : turn) }
      : session)
  };
}

/**
 * 取当前会话。
 *
 * @param store 会话存储
 * @returns 当前会话
 */
export function activeImageSession(store: ImageWorkbenchStore): ImageWorkbenchSession {
  return store.sessions.find((session) => session.id === store.activeId) ?? store.sessions[0];
}

/**
 * 用提示词生成短标题。
 *
 * @param prompt 提示词
 * @returns 不超过 24 个字符的标题
 */
/**
 * 把已完成的提示词接到本轮前面，让后续请求能接着改。
 *
 * @param previous 先前各轮的用户提示词
 * @param prompt 本轮提示词
 * @returns 发给模型的提示词
 */
export function imageFollowUpPrompt(previous: string[], prompt: string): string {
  const history = previous.map((item) => item.trim()).filter(Boolean);
  if (history.length === 0) return prompt;
  const lines = history.map((item, index) => `${index + 1}. ${item}`).join("\n");
  return `此前的生图要求：\n${lines}\n\n请在这些结果上继续：\n${prompt}`;
}

export function titleFromPrompt(prompt: string): string {
  const compact = prompt.replace(/\s+/g, " ").trim();
  return compact.length > 24 ? `${compact.slice(0, 24)}…` : compact || "新的图片";
}

/**
 * 创建只含一个空会话的存储。
 *
 * @returns 初始存储
 */
function createStore(): ImageWorkbenchStore {
  const session = emptySession(new Date().toISOString());
  return { activeId: session.id, sessions: [session] };
}

/**
 * 创建一个空会话。
 *
 * @param now 创建时间
 * @returns 空会话
 */
function emptySession(now: string): ImageWorkbenchSession {
  return { id: `image_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`, title: "新的图片", createdAt: now, updatedAt: now, turns: [] };
}

/**
 * 修改当前会话。
 *
 * @param store 当前存储
 * @param update 会话变换
 * @returns 新存储
 */
function mapActive(store: ImageWorkbenchStore, update: (session: ImageWorkbenchSession) => ImageWorkbenchSession): ImageWorkbenchStore {
  return {
    ...store,
    sessions: store.sessions.map((session) => session.id === store.activeId ? update(session) : session)
  };
}

/**
 * 丢掉无法展示的轮次字段，避免坏数据撑破界面。
 *
 * @param session 读取到的会话
 * @returns 规范化会话
 */
/**
 * 本地存储放不下原图时只保留文字轮次。
 *
 * @param store 当前存储
 * @returns 去掉附图数据的存储
 */
function stripAttachmentData(store: ImageWorkbenchStore): ImageWorkbenchStore {
  return {
    ...store,
    sessions: store.sessions.map((session) => ({
      ...session,
      turns: session.turns.map((turn) => ({ ...turn, attachments: undefined }))
    }))
  };
}

function normalizeSession(session: ImageWorkbenchSession): ImageWorkbenchSession {
  return {
    ...session,
    title: session.title || "新的图片",
    turns: Array.isArray(session.turns) ? session.turns : []
  };
}
