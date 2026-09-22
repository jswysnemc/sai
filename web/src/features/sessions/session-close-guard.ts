import { api, type BackgroundWorkItem } from "../../api/client";

/**
 * 汇总指定会话仍在运行的后台工作，供关闭确认使用。
 *
 * 终端会话和网页会话互相独立。这里只查询正在关闭的那一个会话。
 *
 * @param sessionId 正在关闭的会话
 * @returns 该会话的子智能体和后台命令
 */
export async function loadRunningBackgroundWork(sessionId: string): Promise<BackgroundWorkItem[]> {
  try {
    const response = await api.sessions.backgroundWork(sessionId);
    return response.items ?? [];
  } catch {
    return [];
  }
}

/**
 * 把后台工作写成确认框说明。
 *
 * @param items 运行中的工作
 * @param fallback 没有后台工作时使用的原文
 * @param t 文案函数
 * @returns 确认说明
 */
export function describeRunningBackgroundWork(
  items: BackgroundWorkItem[],
  fallback: string,
  t: (english: string, chinese: string) => string
): string {
  if (items.length === 0) return fallback;
  const subagents = items.filter((item) => item.kind === "subagent").length;
  const commands = items.filter((item) => item.kind === "command").length;
  const names = items.slice(0, 4).map((item) => item.label).join("、");
  const extra = items.length > 4 ? t(`, plus ${items.length - 4} more`, `，另有 ${items.length - 4} 项`) : "";
  return t(
    `${fallback} ${subagents} subagent(s) and ${commands} background command(s) are still running (${names}${extra}). Closing does not stop them.`,
    `${fallback}还有 ${subagents} 个子智能体、${commands} 个后台命令在运行（${names}${extra}）。关闭不会停止它们。`
  );
}
