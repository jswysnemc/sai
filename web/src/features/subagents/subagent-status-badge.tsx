import { subagentStatusLabel } from "./subagent-labels";

/**
 * 渲染子智能体状态徽章。
 *
 * @param props 子智能体状态
 * @returns 状态徽章
 */
export function SubagentStatusBadge({ status }: { status: string }) {
  return <span className={`subagent-status ${status}`}>{subagentStatusLabel(status)}</span>;
}
