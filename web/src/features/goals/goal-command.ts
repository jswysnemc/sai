export type GoalCommand = {
  objective: string;
};

/**
 * 解析输入区 `/goal` 命令。
 *
 * @param value 当前输入文本
 * @returns 命令匹配时返回目标内容，否则返回 null
 */
export function parseGoalCommand(value: string): GoalCommand | null {
  const match = value.trim().match(/^\/goal(?:\s+([\s\S]*))?$/u);
  if (!match) return null;
  return { objective: (match[1] ?? "").trim() };
}
