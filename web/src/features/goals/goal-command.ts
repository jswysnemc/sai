export type GoalCommand = {
  objective: string;
};

/**
 * 解析输入区 `/goal` 命令。
 * 兼容菜单插入的 skill-mention、全角斜杠、无空格中文目标，以及零宽字符。
 *
 * @param value 当前输入文本
 * @returns 命令匹配时返回目标内容，否则返回 null
 */
export function parseGoalCommand(value: string): GoalCommand | null {
  // 1. 去掉 BOM、零宽字符，并规整首尾空白
  let text = value
    .replace(/^\uFEFF/u, "")
    .replace(/[\u200B-\u200D\uFEFF]/gu, "")
    .trim();
  if (!text) return null;

  // 2. 兼容旧版把 /goal 写成 skill-mention 的草稿
  text = text.replace(/^<skill-mention\s+name=["']goal["']\s*><\/skill-mention>/iu, "/goal");

  // 3. 全角斜杠归一成半角
  if (text.startsWith("／")) {
    text = `/${text.slice(1)}`;
  }

  // 4. 大小写不敏感匹配 /goal
  // - `/goal`
  // - `/goal 正文` / `/goal\n正文`
  // - `/goal正文`（中文等非 ASCII 紧贴，避免误伤 /goalie）
  const withSpace = text.match(/^\/goal(?:[\s\u00a0\u3000]+([\s\S]*))?$/iu);
  if (withSpace) {
    return { objective: (withSpace[1] ?? "").trim() };
  }
  const gluedNonAscii = text.match(/^\/goal([^\x00-\x7F][\s\S]*)$/iu);
  if (gluedNonAscii) {
    return { objective: gluedNonAscii[1].trim() };
  }
  return null;
}
