/**
 * 渲染运行中 Agent 的克制脉冲状态。
 *
 * @returns 可访问的运行状态指示器
 */
export function ActiveAgentIndicator() {
  const { t } = useI18n();
  return <span className="active-agent-indicator" role="status" aria-label={t("Agent is working", "Agent 正在工作")}><span /></span>;
}
import { useI18n } from "../i18n/use-i18n";
