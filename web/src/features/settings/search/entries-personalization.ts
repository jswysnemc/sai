import { PROMPT_TEMPLATE_DEFINITIONS } from "../prompts/prompt-template-catalog";
import type { SettingsSearchEntry } from "./settings-search-types";

/** 外观字段：标识、英文标签与中文标签。 */
const APPEARANCE_FIELDS = [
  ["locale", "Interface language", "界面语言"],
  ["theme", "Workspace theme", "工作区主题"],
  ["markdown.preset", "Markdown overall style", "Markdown 整体风格"],
  ["markdown.table.borderStyle", "Table borders", "表格边框"],
  ["markdown.table.density", "Cell density", "单元格密度"],
  ["markdown.table.fullWidth", "Full width tables", "表格占满内容宽度"],
  ["markdown.table.stripedRows", "Striped rows", "斑马纹"],
  ["markdown.table.headerBackground", "Header background", "表头底色"],
  ["markdown.table.wrapCells", "Wrap cell content", "单元格内容换行"],
  ["markdown.codeBlock.fontSize", "Code font size", "代码字体大小"],
  ["markdown.codeBlock.tabSize", "Tab width", "制表符宽度"],
  ["markdown.codeBlock.maxHeight", "Code block maximum height", "代码块最大高度"],
  ["markdown.codeBlock.lineNumbers", "Line numbers", "显示行号"],
  ["markdown.codeBlock.wrapLongLines", "Wrap long lines", "长行换行"],
  ["markdown.codeBlock.showLanguageLabel", "Language label", "语言标签"],
  ["markdown.codeBlock.showCopyButton", "Copy button", "复制按钮"],
  ["markdown.codeBlock.showBorder", "Block border", "代码块外框"]
] as const;

/** 提示词、外观与高级配置的字段索引。 */
export const PERSONALIZATION_SEARCH_ENTRIES: SettingsSearchEntry[] = [
  ...APPEARANCE_FIELDS.map(([key, labelEn, labelZh]): SettingsSearchEntry => ({ anchor: `appearance.${key}`, section: "appearance", labelEn, labelZh, keywords: [key] })),
  ...PROMPT_TEMPLATE_DEFINITIONS.flatMap((definition) => (["system", "user"] as const).map((field): SettingsSearchEntry => ({
    anchor: `prompts.${definition.id}.${field}`, section: "prompts",
    labelEn: `${definition.labelEn} · ${field === "system" ? "System instruction" : "Input template"}`,
    labelZh: `${definition.labelZh} · ${field === "system" ? "系统指令" : "输入模板"}`,
    keywords: [`prompt.templates.${definition.id}.${field}`, ...definition.variables.map((variable) => variable.name)]
  }))),
  { anchor: "advanced.json", section: "advanced", labelEn: "Configuration document", labelZh: "配置文档", keywords: ["JSON", "格式化", "format", "validation", "校验"] }
];
