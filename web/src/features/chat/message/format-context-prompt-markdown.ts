import { text, type Locale } from "../../i18n/locale";

/**
 * 将会话上下文原始文本整理为更适合 UI 的 Markdown。
 * 1. 拆出 instruction-files / available-skills 等 XML 段
 * 2. 把文件路径与范围提成标题
 * 3. 保留正文 Markdown，避免整段糊成一堵墙
 *
 * @param source 后端返回的原始上下文文本
 * @param locale 当前界面语言
 * @returns 可交给 MarkdownRenderer 的文本
 */
export function formatContextPromptMarkdown(source: string, locale: Locale = "zh-CN"): string {
  const raw = source.replace(/\r\n/g, "\n").trim();
  if (!raw) return "";

  const parts: string[] = [];
  let cursor = 0;
  const blockPattern =
    /<(instruction-files|available-skills|loaded_tools|system-reminder|selected-model|goal-continuation|associative-memory|active-goal)(?:\s[^>]*)?>([\s\S]*?)<\/\1>/giu;

  for (const match of raw.matchAll(blockPattern)) {
    const index = match.index ?? 0;
    if (index > cursor) {
      parts.push(formatLooseSegment(raw.slice(cursor, index), locale));
    }
    const tag = match[1].toLowerCase();
    const body = match[2] ?? "";
    if (tag === "instruction-files") {
      parts.push(formatInstructionFiles(body, locale));
    } else if (tag === "available-skills") {
      parts.push(
        formatNamedXmlSection(
          text(locale, "Available skills", "技能目录"),
          "available-skills",
          body,
          locale
        )
      );
    } else if (tag === "loaded_tools") {
      parts.push(
        formatNamedXmlSection(
          text(locale, "Loaded tools", "已加载工具"),
          "loaded_tools",
          body,
          locale
        )
      );
    } else if (tag === "system-reminder") {
      parts.push(
        formatNamedXmlSection(
          text(locale, "System reminder", "系统提醒"),
          "system-reminder",
          body,
          locale
        )
      );
    } else if (tag === "selected-model") {
      parts.push(
        formatNamedXmlSection(
          text(locale, "Selected model", "当前模型"),
          "selected-model",
          body,
          locale
        )
      );
    } else if (tag === "active-goal") {
      parts.push(
        formatNamedXmlSection(
          text(locale, "Active Goal", "活动 Goal"),
          "active-goal",
          body,
          locale
        )
      );
    } else if (tag === "associative-memory") {
      parts.push(
        formatNamedXmlSection(
          text(locale, "Associative memory", "关联记忆"),
          "associative-memory",
          body,
          locale
        )
      );
    } else {
      parts.push(formatNamedXmlSection(tag, tag, body, locale));
    }
    cursor = index + match[0].length;
  }

  if (cursor < raw.length) {
    parts.push(formatLooseSegment(raw.slice(cursor), locale));
  }

  return parts
    .map((part) => part.trim())
    .filter(Boolean)
    .join("\n\n");
}

/**
 * 格式化 instruction-files 段。
 *
 * @param body 标签内正文
 * @param locale 当前界面语言
 * @returns Markdown
 */
function formatInstructionFiles(body: string, locale: Locale): string {
  const intro: string[] = [];
  const files: string[] = [];
  const rest = body.trim();
  const filePattern =
    /<instruction-file\s+([^>]+)>([\s\S]*?)<\/instruction-file>/giu;
  let lastIndex = 0;
  for (const match of rest.matchAll(filePattern)) {
    const index = match.index ?? 0;
    if (index > lastIndex) {
      const head = rest.slice(lastIndex, index).trim();
      if (head) intro.push(head);
    }
    const attrs = parseXmlAttributes(match[1] ?? "");
    const content = (match[2] ?? "").trim();
    const scope = attrs.scope || "project";
    const path = attrs.path || "unknown";
    const scopeLabel =
      scope === "global"
        ? text(locale, "global", "全局")
        : scope === "project"
          ? text(locale, "project", "项目")
          : scope;
    files.push(
      [
        `### ${escapeInline(path)}`,
        "",
        `- ${text(locale, "Scope", "范围")}${text(locale, ": ", "：")}\`${escapeInline(scopeLabel)}\``,
        `- ${text(locale, "Path", "路径")}${text(locale, ": ", "：")}\`${escapeInline(path)}\``,
        "",
        content || text(locale, "_(empty file)_", "_（空文件）_")
      ].join("\n")
    );
    lastIndex = index + match[0].length;
  }
  if (lastIndex < rest.length) {
    const tail = rest.slice(lastIndex).trim();
    if (tail) intro.push(tail);
  }

  const sections = [`## ${text(locale, "Instruction files", "指令文件")}`, ""];
  if (intro.length > 0) {
    sections.push(intro.join("\n\n"), "");
  }
  if (files.length === 0) {
    sections.push(text(locale, "_No instruction-file subsections found_", "_未找到 instruction-file 子段_"));
  } else {
    sections.push(...files);
  }
  return sections.join("\n");
}

/**
 * 将具名 XML 段渲染为 Markdown 小节。
 *
 * @param title 本地化标题
 * @param tag 原始标签名
 * @param body 正文
 * @param locale 当前界面语言
 * @returns Markdown
 */
function formatNamedXmlSection(title: string, tag: string, body: string, locale: Locale): string {
  const content = body.trim();
  if (!content) {
    return `## ${title}\n\n${text(locale, "_(empty)_", "_（空）_")}`;
  }
  // 技能目录等本身常是 Markdown / 列表，直接展示
  if (looksLikeMarkdown(content) || !content.includes("<")) {
    return `## ${title}\n\n${content}`;
  }
  return [
    `## ${title}`,
    "",
    "```xml",
    `<${tag}>`,
    content,
    `</${tag}>`,
    "```"
  ].join("\n");
}

/**
 * 格式化不在已知 XML 段内的文本。
 *
 * @param segment 原文片段
 * @param locale 当前界面语言
 * @returns Markdown
 */
function formatLooseSegment(segment: string, locale: Locale): string {
  const value = segment.trim();
  if (!value) return "";

  // 已有 Markdown 标题 / 工具区时保持原样
  if (looksLikeMarkdown(value)) {
    return value;
  }

  // 孤立 instruction-file
  if (/<instruction-file\b/i.test(value)) {
    return formatInstructionFiles(value, locale);
  }

  // 其它尖括号标签包一层代码块，避免糊成一行
  if (/<[a-zA-Z][\w:-]*[\s>]/.test(value) && value.includes(">")) {
    return ["```text", value, "```"].join("\n");
  }

  return value;
}

/**
 * 粗判文本是否已是 Markdown。
 *
 * @param value 文本
 * @returns 是否像 Markdown
 */
function looksLikeMarkdown(value: string): boolean {
  return /(?:^|\n)\s{0,3}(#{1,6}\s|[-*]\s|\d+\.\s|```|\|)/u.test(value);
}

/**
 * 解析简单 XML 属性。
 *
 * @param raw 属性字符串
 * @returns 属性表
 */
function parseXmlAttributes(raw: string): Record<string, string> {
  const result: Record<string, string> = {};
  const pattern = /([\w:-]+)\s*=\s*"([^"]*)"/gu;
  for (const match of raw.matchAll(pattern)) {
    result[match[1]] = match[2];
  }
  return result;
}

/**
 * 转义行内反引号内容中的危险字符。
 *
 * @param value 原文
 * @returns 安全文本
 */
function escapeInline(value: string): string {
  return value.replace(/`/g, "'");
}
