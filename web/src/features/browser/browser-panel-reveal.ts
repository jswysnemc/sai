import type { WebEvent } from "../../api/contracts/sessions";
import { MOBILE_WORKBENCH_MEDIA_QUERY } from "../workspace/mobile-workbench-state";
import { OPEN_WORKSPACE_PANEL_EVENT } from "../workspace/workspace-panel-options";

/** 已经自动打开过浏览器面板的运行，避免同一轮反复抢占用户的面板选择。 */
const revealedRuns = new Set<string>();
/** 记录上限，长时间运行的页面不无限累积。 */
const MAX_REMEMBERED_RUNS = 64;

/**
 * 判断工具名是否属于内置浏览器工具组。
 *
 * @param name 工具名
 * @returns 浏览器工具时为 true
 */
export function isBrowserToolName(name: unknown): boolean {
  return typeof name === "string" && name.startsWith("browser_");
}

/**
 * Agent 在一轮运行中首次调用浏览器工具时，自动在工作区打开浏览器面板。
 *
 * 只处理实时事件且只在桌面布局生效；移动端打开面板会把聊天区切走，交给用户手动打开。
 *
 * @param event 会话实时事件
 * @returns 本次触发了打开时为 true
 */
export function revealBrowserPanelForEvent(event: WebEvent): boolean {
  if (event.replayed || event.type !== "tool.call.started") return false;
  if (!isBrowserToolName(event.payload.name)) return false;
  const runId = event.run_id || event.session_id;
  if (revealedRuns.has(runId)) return false;
  if (window.matchMedia(MOBILE_WORKBENCH_MEDIA_QUERY).matches) return false;
  revealedRuns.add(runId);
  if (revealedRuns.size > MAX_REMEMBERED_RUNS) {
    const oldest = revealedRuns.values().next().value;
    if (oldest !== undefined) revealedRuns.delete(oldest);
  }
  window.dispatchEvent(new CustomEvent(OPEN_WORKSPACE_PANEL_EVENT, { detail: { tab: "browser" } }));
  return true;
}

/** 仅供测试：清空已打开记录。 */
export function resetBrowserPanelRevealForTests() {
  revealedRuns.clear();
}
