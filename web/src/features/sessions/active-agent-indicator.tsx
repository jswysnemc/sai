/**
 * 渲染运行中 Agent 的克制脉冲状态。
 *
 * @returns 可访问的运行状态指示器
 */
export function ActiveAgentIndicator() {
  return <span className="active-agent-indicator" role="status" aria-label="Agent 正在工作"><span /></span>;
}
