import { text, type Locale } from "../../i18n/locale";

/**
 * 构造外观预览使用的示例 Markdown。
 *
 * 示例需要同时覆盖表格与代码块的全部可调项：多列表格用于观察边框、密度、
 * 斑马纹与表头底色；含长行和缩进的代码用于观察行号、折行、制表符宽度。
 *
 * @param locale 当前界面语言
 * @returns 可直接交给 MarkdownRenderer 的示例文本
 */
export function buildAppearancePreviewSample(locale: Locale): string {
  const table = [
    `| ${text(locale, "Model", "模型")} | ${text(locale, "Context", "上下文")} | ${text(locale, "Status", "状态")} |`,
    "| --- | --- | --- |",
    `| deepseek-v4-pro | 1000k | ${text(locale, "Active", "使用中")} |`,
    `| deepseek-v4-flash | 1000k | ${text(locale, "Standby", "备用")} |`,
    `| claude-opus-4-8 | 200k | ${text(locale, "Standby", "备用")} |`
  ].join("\n");

  // 第三行刻意留长，用于观察「长行换行」与横向滚动的差异
  const code = [
    "```typescript",
    "function resolveModel(name: string): Model | undefined {",
    "  const registry = loadRegistry();",
    "  return registry.find((item) => item.name === name && item.enabled && item.contextWindow > 0);",
    "}",
    "```"
  ].join("\n");

  return `${table}\n\n${code}`;
}
