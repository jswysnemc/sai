import type { AppConfig } from "../../../api/contracts";
import { BUILTIN_AGENT_PROFILES } from "../agents/agent-profile-state";
import { hookItemKey } from "../hooks/hook-templates";
import { fieldLabel } from "../structured-field-metadata";
import type { SettingsSearchEntry } from "./settings-search-types";

type Field = readonly [key: string, en: string, zh: string];

/**
 * 为一个分区生成共享字段索引。
 * @param section 分区标识
 * @param fields 字段键及中英文标签
 * @returns 字段条目
 */
function entries(section: SettingsSearchEntry["section"], fields: readonly Field[]): SettingsSearchEntry[] {
  return fields.map(([key, labelEn, labelZh]) => ({ section, anchor: `${section}.${key}`, labelEn, labelZh, keywords: [`${section}.${key}`] }));
}

/** 不依赖所选对象的字段。 */
export const AGENT_CAPABILITY_ENTRIES: SettingsSearchEntry[] = [
  ...entries("agents", [["default_agent", "Default Web Agent", "网页默认 Agent"], ["tui_agent", "Default TUI Agent", "交互终端默认 Agent"], ["cli_agent", "Default CLI Agent", "命令行默认 Agent"], ["gateway_agent", "Default gateway Agent", "网关默认 Agent"]]),
  ...entries("hooks", [["enabled", "Enable hooks", "启用 Hooks"]]),
  ...entries("memory", [["enabled", "Enable memory", "启用记忆"], ["search", "Search memories", "搜索记忆"], ["type", "Memory type", "记忆类型"], ["scope", "Memory scope", "记忆作用域"]]),
  ...entries("mcp", [["enabled", "Enable MCP", "启用 MCP"], ["id", "Server ID", "服务 ID"], ["transport", "Transport", "传输方式"], ["timeout_ms", "Timeout", "超时"], ["command", "Command", "命令"], ["cwd", "Working directory", "工作目录"], ["args", "Arguments", "参数"], ["env", "Environment", "环境变量"], ["url", "Server address", "服务地址"], ["message_url", "Message address", "消息地址"], ["headers", "Request headers", "请求头"]]),
  ...entries("gateways", [
    ["qq.transport", "QQ transport", "QQ 传输方式"], ["qq.listen", "QQ listen address", "QQ 监听地址"], ["qq.base_url", "QQ API address", "QQ API 地址"], ["qq.app_id", "QQ App ID", "QQ 应用 ID"], ["qq.client_secret", "QQ client secret", "QQ 客户端密钥"], ["qq.token", "QQ token", "QQ 兼容令牌"],
    ["weixin.base_url", "Weixin API address", "微信 API 地址"], ["weixin.cdn_base_url", "Weixin CDN address", "微信 CDN 地址"], ["weixin.bot_type", "Weixin bot type", "微信机器人类型"], ["weixin.account", "Weixin account", "微信账户"], ["weixin.bot_agent", "Weixin Agent", "微信 Agent"], ["weixin.token", "Weixin access token", "微信访问令牌"]
  ])
];

const PROFILE_FIELDS: Field[] = [["name", "Display name", "显示名称"], ["description", "Purpose", "用途描述"], ["model", "Model", "模型"], ["thinking_level", "Thinking level", "思考等级"], ["system_prompt", "System prompt", "系统提示词"], ["enabled_tools", "Tool permissions", "工具权限"], ["skills", "Skill permissions", "技能权限"]];
const HOOK_FIELDS: Field[] = [["name", "Name", "名称"], ["event", "Event", "事件"], ["kind", "Action type", "动作类型"], ["timeout_ms", "Timeout", "超时"]];

/**
 * 【Web 设置】【对象字段索引】索引对象名称和字段键，不采集正文或凭据。
 * @param config 当前应用配置
 * @returns Agent、Hook 与技能行为条目
 */
export function buildCapabilitySearchEntries(config?: AppConfig | null): SettingsSearchEntry[] {
  const profiles = new Map([["default", { id: "default", name: "Default Agent" }], ...BUILTIN_AGENT_PROFILES.map((profile) => [profile.id, profile] as const), ...(config?.agents ?? []).map((profile) => [profile.id, profile] as const)]);
  return [
    ...[...profiles.values()].flatMap((profile) => entries("agents", PROFILE_FIELDS).map((entry) => ({ ...entry, item: profile.id, view: entry.anchor.endsWith("enabled_tools") ? "tools" : entry.anchor.endsWith("skills") ? "skills" : "basic", labelEn: `${profile.name || profile.id} · ${entry.labelEn}`, labelZh: `${profile.name || profile.id} · ${entry.labelZh}`, keywords: [...entry.keywords ?? [], profile.id] }))),
    ...(config?.hooks?.items ?? []).flatMap((hook, index) => entries("hooks", [...HOOK_FIELDS, hook.kind === "http" ? ["requests", "HTTP requests", "HTTP 请求"] : ["script", "Script", "脚本"]]).map((entry) => ({ ...entry, item: hookItemKey(hook, index), labelEn: `${hook.name} · ${entry.labelEn}`, labelZh: `${hook.name} · ${entry.labelZh}` }))),
    ...Object.keys(config?.skills ?? {}).map((key) => ({ section: "skills" as const, view: "behavior", anchor: `skills.${key}`, labelEn: fieldLabel(key, (en) => en), labelZh: fieldLabel(key, (_en, zh) => zh), keywords: [`skills.${key}`] }))
  ];
}
