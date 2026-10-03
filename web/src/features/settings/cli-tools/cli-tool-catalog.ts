import {
  Calculator,
  Database,
  Globe,
  ScanEye,
  Sparkles
} from "../../../shared/ui/icons";
import type { LucideIcon } from "../../../shared/ui/icons";
import type { Locale } from "../../i18n/locale";

export type CliToolCategoryId = "research" | "media" | "knowledge" | "utility" | "system";

export type CliToolCatalogEntry = {
  id: string;
  labelEn: string;
  labelZh: string;
  descriptionEn: string;
  descriptionZh: string;
  category: CliToolCategoryId;
  icon: LucideIcon;
};

const CLI_TOOL_CATALOG: Record<string, CliToolCatalogEntry> = {
  web: tool("web", "Web search", "网页搜索", "Built-in search routing, endpoints, and credentials", "内置搜索路由、供应商地址与凭据", "research", Globe),
  vision: tool("vision", "Vision", "视觉理解", "Image understanding and terminal preview", "图片理解与终端预览", "media", ScanEye),
  calculator: tool("calculator", "Calculator", "计算器", "Local mathematical calculations", "本地数学计算", "utility", Calculator),
  memory: tool("memory", "Long-term memory", "长期记忆", "Read, write and delete memory files", "读写与删除记忆文件", "knowledge", Database)
};

/**
 * 读取 CLI 助手工具的展示元数据。
 *
 * @param id 历史配置中的工具标识
 * @returns 已知工具元数据；未知工具返回可读的通用元数据
 */
export function getCliToolCatalogEntry(id: string): CliToolCatalogEntry {
  return CLI_TOOL_CATALOG[id] ?? tool(
    id,
    readableIdentifier(id),
    readableIdentifier(id),
    "Optional capability exposed to CLI assistants",
    "可向 CLI 助手开放的可选能力",
    "utility",
    Sparkles
  );
}

/**
 * 返回当前语言下的工具名称。
 *
 * @param entry 工具元数据
 * @param locale 当前界面语言
 * @returns 本地化工具名称
 */
export function cliToolLabel(entry: CliToolCatalogEntry, locale: Locale): string {
  return locale === "zh-CN" ? entry.labelZh : entry.labelEn;
}

/**
 * 返回当前语言下的工具说明。
 *
 * @param entry 工具元数据
 * @param locale 当前界面语言
 * @returns 本地化工具说明
 */
export function cliToolDescription(entry: CliToolCatalogEntry, locale: Locale): string {
  return locale === "zh-CN" ? entry.descriptionZh : entry.descriptionEn;
}

/**
 * 返回当前语言下的工具类别。
 *
 * @param category 工具类别标识
 * @param locale 当前界面语言
 * @returns 本地化类别名称
 */
export function cliToolCategoryLabel(category: CliToolCategoryId, locale: Locale): string {
  const labels: Record<CliToolCategoryId, [string, string]> = {
    research: ["Research", "检索研究"],
    media: ["Media", "多媒体"],
    knowledge: ["Knowledge", "知识"],
    utility: ["Utilities", "实用工具"],
    system: ["System", "系统"]
  };
  return labels[category][locale === "zh-CN" ? 1 : 0];
}

/**
 * 构造一项固定的工具目录元数据。
 *
 * @param id 工具标识
 * @param labelEn 英文名称
 * @param labelZh 中文名称
 * @param descriptionEn 英文说明
 * @param descriptionZh 中文说明
 * @param category 工具类别
 * @param icon 图标组件
 * @returns 工具目录项
 */
function tool(
  id: string,
  labelEn: string,
  labelZh: string,
  descriptionEn: string,
  descriptionZh: string,
  category: CliToolCategoryId,
  icon: LucideIcon
): CliToolCatalogEntry {
  return { id, labelEn, labelZh, descriptionEn, descriptionZh, category, icon };
}

/**
 * 将配置标识转换为可读名称。
 *
 * @param value 配置标识
 * @returns 使用空格分隔并首字母大写的名称
 */
function readableIdentifier(value: string): string {
  const text = value.replaceAll("_", " ").trim();
  return text ? text.charAt(0).toUpperCase() + text.slice(1) : value;
}
