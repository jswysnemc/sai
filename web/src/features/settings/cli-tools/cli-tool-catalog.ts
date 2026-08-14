import {
  BookOpen,
  Calculator,
  CircleGauge,
  Database,
  Gamepad2,
  Image,
  Images,
  Library,
  MonitorCog,
  PackageSearch,
  ScanEye,
  Search,
  Sparkles,
  TerminalSquare,
  WandSparkles
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
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
  weather: tool("weather", "Weather", "天气查询", "Current weather and forecasts", "当前天气与预报", "research", Search),
  web_images: tool("web_images", "Web images", "网页图片", "Image search, download, and review", "图片搜索、下载与审核", "research", Images),
  deep_research: tool("deep_research", "Deep research", "深度研究", "Long-form research with review rounds", "长篇研究与多轮审阅", "research", Library),
  deep_diagnose: tool("deep_diagnose", "Deep diagnosis", "深度诊断", "Multi-round diagnosis and correction", "多轮诊断与修正", "system", CircleGauge),
  vision: tool("vision", "Vision", "视觉理解", "Image understanding and terminal preview", "图片理解与终端预览", "media", ScanEye),
  exchange_rate: tool("exchange_rate", "Exchange rates", "汇率查询", "Currency exchange rate lookup", "货币汇率查询", "research", Calculator),
  xuanxue: tool("xuanxue", "I Ching", "六十四卦", "I Ching reference tools", "六十四卦参考工具", "knowledge", BookOpen),
  image_generation: tool("image_generation", "Image generation", "图片生成", "Generate images from text prompts", "根据文本提示生成图片", "media", WandSparkles),
  print_image: tool("print_image", "Image output", "图片输出", "Render image files in supported terminals", "在支持的终端中显示图片", "media", Image),
  memes: tool("memes", "Meme gallery", "表情图库", "Persona-aware local image library", "按人格选择本地图片库", "media", Images),
  knowledge_base: tool("knowledge_base", "Knowledge base", "知识库", "Local files and semantic retrieval", "本地文件与语义检索", "knowledge", Database),
  archlinux: tool("archlinux", "Arch Linux", "Arch Linux", "ArchWiki and AUR lookup", "ArchWiki 与 AUR 查询", "knowledge", PackageSearch),
  man: tool("man", "Online manuals", "在线手册", "Search and read online manual pages", "搜索并阅读在线手册", "knowledge", TerminalSquare),
  moegirl: tool("moegirl", "Moegirlpedia", "萌娘百科", "Moegirlpedia content lookup", "萌娘百科内容查询", "knowledge", BookOpen),
  hash_codec: tool("hash_codec", "Hash codec", "哈希编码", "Hashing and common text codecs", "哈希与常用文本编码", "utility", Sparkles),
  calculator: tool("calculator", "Calculator", "计算器", "Local mathematical calculations", "本地数学计算", "utility", Calculator),
  package_advisor: tool("package_advisor", "Package advisor", "软件包建议", "Package review and installation guidance", "软件包审查与安装建议", "system", PackageSearch),
  linux_game_compatibility: tool("linux_game_compatibility", "Linux game compatibility", "Linux 游戏兼容性", "Proton and anti-cheat compatibility lookup", "Proton 与反作弊兼容性查询", "research", Gamepad2),
  diagnostics: tool("diagnostics", "Runtime diagnostics", "运行诊断", "Bounded system diagnostic commands", "受限的系统诊断命令", "system", MonitorCog),
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
