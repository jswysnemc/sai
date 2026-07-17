/**
 * 返回子智能体状态的中文标签。
 *
 * @param status 子智能体状态
 * @returns 状态中文名称
 */
export function subagentStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    running: "运行中",
    completed: "已完成",
    failed: "失败",
    cancelled: "已取消"
  };
  return labels[status] ?? status;
}

/**
 * 返回子智能体类型的中文标签。
 *
 * @param type 子智能体类型
 * @returns 类型中文名称
 */
export function subagentTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    general: "通用",
    explore: "探索"
  };
  return labels[type] ?? type;
}

/**
 * 计算子智能体运行时长的可读文本。
 *
 * @param startedAt 起始 Unix 秒
 * @param updatedAt 最近更新 Unix 秒
 * @returns 运行时长文本
 */
export function subagentDuration(startedAt: number, updatedAt: number): string {
  const seconds = Math.max(0, updatedAt - startedAt);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return rest > 0 ? `${minutes}m ${rest}s` : `${minutes}m`;
}
