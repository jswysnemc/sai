/** 请求聚焦某个子智能体的事件名。 */
export const FOCUS_SUBAGENT_EVENT = "sai:focus-subagent";

/**
 * 待聚焦的子智能体标识。
 *
 * 概览里点击条目时子智能体面板可能尚未挂载，事件会打空。
 * 这里留一份待取值，面板首次渲染时自行认领。
 */
let pendingFocusId: string | null = null;

/**
 * 请求打开指定子智能体的详情。
 *
 * @param id 子智能体标识
 * @returns 无
 */
export function requestSubagentFocus(id: string): void {
  pendingFocusId = id;
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(FOCUS_SUBAGENT_EVENT, { detail: { id } }));
}

/**
 * 取走待聚焦的子智能体标识。
 *
 * @returns 待聚焦标识；没有时返回 null
 */
export function takePendingSubagentFocus(): string | null {
  const id = pendingFocusId;
  pendingFocusId = null;
  return id;
}

/**
 * 从聚焦事件中读出子智能体标识。
 *
 * @param event 事件对象
 * @returns 子智能体标识；载荷不合法时返回 null
 */
export function focusIdFromEvent(event: Event): string | null {
  const detail = (event as CustomEvent<{ id?: string }>).detail;
  return typeof detail?.id === "string" && detail.id.length > 0 ? detail.id : null;
}
