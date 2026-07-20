/** 解析后的 Skill 文档结构。 */
export type ParsedSkillDocument = {
  name: string;
  description: string;
  body: string;
  /** 是否检测到 YAML frontmatter 块。 */
  hasFrontmatter: boolean;
};

/**
 * 从 SKILL.md 原文解析 name、description 与正文。
 *
 * @param content 完整文档
 * @returns 解析结果；无 frontmatter 时 name/description 为空，body 为全文
 */
export function parseSkillDocument(content: string): ParsedSkillDocument {
  const normalized = content.replace(/\r\n/g, "\n");
  if (!normalized.startsWith("---\n") && normalized !== "---") {
    return { name: "", description: "", body: content, hasFrontmatter: false };
  }
  const end = normalized.indexOf("\n---", 4);
  if (end < 0) {
    return { name: "", description: "", body: content, hasFrontmatter: false };
  }
  const frontmatter = normalized.slice(4, end);
  const body = normalized.slice(end + 4).replace(/^\n+/u, "").replace(/\n+$/u, "");
  return {
    name: readFrontmatterField(frontmatter, "name"),
    description: readFrontmatterField(frontmatter, "description"),
    body,
    hasFrontmatter: true
  };
}

/**
 * 用独立字段拼回完整 SKILL.md。
 *
 * @param name Skill 名称
 * @param description Skill 描述
 * @param body Markdown 正文（不含 frontmatter）
 * @returns 完整文档
 */
export function composeSkillDocument(name: string, description: string, body: string): string {
  const safeName = name.trim() || "unnamed-skill";
  const safeDescription = description.trim() || "Describe when this Skill should be used";
  const normalizedBody = body.replace(/\r\n/g, "\n").replace(/^\n+/, "").replace(/\n+$/, "");
  return `---\nname: ${escapeYamlScalar(safeName)}\ndescription: ${escapeYamlScalar(safeDescription)}\n---\n\n${normalizedBody}\n`;
}

/**
 * 读取 frontmatter 中的简单标量字段。
 *
 * @param frontmatter YAML 段（不含 ---）
 * @param key 字段名
 * @returns 字段值
 */
function readFrontmatterField(frontmatter: string, key: string): string {
  const pattern = new RegExp(`^${key}:\\s*(.*)$`, "imu");
  const match = frontmatter.match(pattern);
  if (!match) return "";
  let value = (match[1] ?? "").trim();
  if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
    value = value.slice(1, -1);
  }
  return value.trim();
}

/**
 * 将标量写成可放入 YAML 的单行值。
 *
 * @param value 原始文本
 * @returns 转义后的 YAML 标量
 */
function escapeYamlScalar(value: string): string {
  if (/[:#\[\]{},&*!|>'"%@`]|^\s|\s$|\n/.test(value)) {
    return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
  }
  return value;
}
