import { subagentStatusLabel } from "./subagent-labels";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染子智能体状态徽章。
 *
 * @param props 子智能体状态
 * @returns 状态徽章
 */
export function SubagentStatusBadge({ status }: { status: string }) {
  const { locale } = useI18n();
  return <span className={`subagent-status ${status}`}>{subagentStatusLabel(status, locale)}</span>;
}
