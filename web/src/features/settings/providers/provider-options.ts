import type { ProviderConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 返回供应商协议下拉选项。
 *
 * @returns 协议选项列表
 */
export function protocolOptions() {
  const { t } = useI18n();
  return [
    { value: "auto", label: t("Auto detect", "自动检测") },
    { value: "openai-chat", label: "OpenAI Chat Completions" },
    { value: "openai-responses", label: "OpenAI Responses" },
    { value: "anthropic", label: "Anthropic Messages" }
  ];
}

/**
 * 返回思考格式下拉选项。
 *
 * 取值必须与后端白名单（src/config/app.rs 的 thinking_format 校验）一致，
 * 否则保存时会被拒绝。
 *
 * @returns 思考格式选项列表
 */
export function thinkingFormatOptions() {
  const { t } = useI18n();
  return [
    { value: "auto", label: t("Automatic", "自动") },
    { value: "openai-chat-reasoning-effort", label: "openai-chat-reasoning-effort" },
    { value: "reasoning", label: "reasoning" },
    { value: "anthropic-thinking", label: "anthropic-thinking" },
    { value: "deepseek-thinking", label: "deepseek-thinking" },
    { value: "moonshot-thinking", label: "moonshot-thinking" },
    { value: "string", label: "string" },
    { value: "object", label: "object" },
    { value: "disabled", label: t("Disabled", "停用") }
  ];
}

/** 思考等级选项；取值与后端枚举一致。 */
export const THINKING_OPTIONS = [
  { value: "auto", label: "auto" },
  { value: "max", label: "max" },
  { value: "xhigh", label: "xhigh" },
  { value: "high", label: "high" },
  { value: "medium", label: "medium" },
  { value: "low", label: "low" },
  { value: "none", label: "none" }
];

/**
 * 判断客户端模拟是否为 Claude Code。
 *
 * @param style 客户端模拟配置
 * @returns Claude 模拟时 true
 */
export function isClaudeClientStyle(style?: string): boolean {
  const normalized = (style ?? "auto").trim().toLowerCase();
  return normalized === "claude" || normalized === "claude-code" || normalized === "claude_code";
}

/**
 * 判断客户端模拟是否使用 OpenAI Responses 兼容头。
 *
 * @param style 客户端模拟配置
 * @returns Codex 模拟时 true
 */
export function isCodexClientStyle(style?: string): boolean {
  const normalized = (style ?? "auto").trim().toLowerCase();
  return normalized === "codex" || normalized === "codex-cli";
}

/**
 * 返回客户端模拟的 User-Agent 占位符。
 *
 * @param provider 供应商配置
 * @returns 占位符文本
 */
export function userAgentPlaceholder(provider: ProviderConfig): string {
  if (isCodexClientStyle(provider.client_style)) return "codex_cli_rs/0.144.0";
  if (isClaudeClientStyle(provider.client_style)) return "claude-cli/2.1.113 (external, cli)";
  return "sai/0.1";
}
