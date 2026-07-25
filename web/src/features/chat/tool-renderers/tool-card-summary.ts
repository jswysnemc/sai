import { toolDisplaySummary } from "./tool-display-summary";
import type { Locale } from "../../i18n/locale";

/**
 * 提取适合在折叠工具卡头部展示的参数摘要。
 *
 * @param name 工具名称
 * @param argumentsText 工具参数 JSON 或参数预览
 * @param locale 界面语言
 * @param workspacePath 当前工作区路径，用于相对路径展示
 * @returns 命令、路径、搜索词等紧凑摘要
 */
export function toolCardSummary(
  name: string,
  argumentsText: string,
  locale: Locale = "zh-CN",
  workspacePath = ""
): string {
  return toolDisplaySummary(name, argumentsText, locale, workspacePath);
}
