import {
  BarChart3,
  Bot,
  Brain,
  Braces,
  Cable,
  GitBranch,
  KeyRound,
  Palette,
  Plug,
  Server,
  SlidersHorizontal,
  Sparkles,
  Webhook
} from "lucide-react";
import type {
  SettingsGroupId,
  SettingsGroupMeta,
  SettingsSectionId,
  SettingsSectionMeta,
  SettingsSurfaceKind
} from "./settings-types";

/** 默认打开的设置 section。 */
export const DEFAULT_SETTINGS_SECTION: SettingsSectionId = "providers";

/**
 * 侧栏分组顺序与文案。
 */
export const SETTINGS_GROUPS: SettingsGroupMeta[] = [
  { id: "general", labelEn: "General", labelZh: "常用配置" },
  { id: "integrations", labelEn: "Extensions", labelZh: "扩展与集成" },
  { id: "workspace", labelEn: "Workspace", labelZh: "工作区" },
  { id: "operations", labelEn: "Data and ops", labelZh: "数据与运维" },
  { id: "advanced", labelEn: "Advanced", labelZh: "高级" }
];

/**
 * 设置页 section 注册表。
 *
 * 新增 section：
 * 1. 在 SettingsSectionId 联合类型中补充 id
 * 2. 在本数组追加元数据
 * 3. 在 SettingsSectionBody 中挂载组件
 */
export const SETTINGS_SECTIONS: SettingsSectionMeta[] = [
  {
    id: "providers",
    group: "general",
    kind: "app-config",
    labelEn: "Providers and models",
    labelZh: "供应商与模型",
    descriptionEn: "Endpoints, credentials, and model lists",
    descriptionZh: "接口、凭据和模型列表",
    icon: KeyRound,
    searchKeys: ["provider", "model", "api_key", "base_url", "供应商", "模型", "凭据"]
  },
  {
    id: "agents",
    group: "general",
    kind: "app-config",
    labelEn: "Agent profiles",
    labelZh: "Agent 配置",
    descriptionEn: "Prompts, tools, and skill exposure",
    descriptionZh: "系统提示词、工具与技能暴露",
    icon: Bot,
    searchKeys: ["agent", "prompt", "tool", "skill", "权限"]
  },
  {
    id: "runtime",
    group: "general",
    kind: "app-config",
    labelEn: "Runtime",
    labelZh: "运行时",
    descriptionEn: "Permissions, notifications, terminal, and display",
    descriptionZh: "权限、通知、终端与显示",
    icon: SlidersHorizontal,
    searchKeys: ["runtime", "permission", "notification", "terminal", "context", "display", "tools", "权限", "通知", "终端", "上下文"]
  },
  {
    id: "appearance",
    group: "general",
    kind: "client-pref",
    labelEn: "Appearance",
    labelZh: "外观",
    descriptionEn: "Language, theme, and colors",
    descriptionZh: "界面语言、主题和颜色方案",
    icon: Palette,
    searchKeys: ["theme", "language", "locale", "appearance", "主题", "语言", "配色"]
  },
  {
    id: "plugins",
    group: "integrations",
    kind: "app-config",
    labelEn: "Plugins",
    labelZh: "插件",
    descriptionEn: "Search, vision, knowledge, and memory",
    descriptionZh: "搜索、视觉、知识库和记忆",
    icon: Plug,
    searchKeys: ["plugin", "search", "vision", "knowledge", "插件"]
  },
  {
    id: "skills",
    group: "integrations",
    kind: "operations",
    labelEn: "Skills",
    labelZh: "Skills",
    descriptionEn: "Scan, edit, create, and enable Skills",
    descriptionZh: "扫描、编辑、新增与启停 Skills",
    icon: Sparkles,
    searchKeys: ["skill", "skills", "SKILL.md", "技能"]
  },
  {
    id: "mcp",
    group: "integrations",
    kind: "local-config",
    labelEn: "MCP",
    labelZh: "MCP",
    descriptionEn: "External Model Context Protocol servers",
    descriptionZh: "外部 MCP 工具服务",
    icon: Server,
    searchKeys: ["mcp", "stdio", "sse", "server", "工具服务"]
  },
  {
    id: "hooks",
    group: "integrations",
    kind: "app-config",
    labelEn: "Hooks",
    labelZh: "Hooks",
    descriptionEn: "Lifecycle shell and HTTP actions",
    descriptionZh: "生命周期 shell 与 HTTP 动作",
    icon: Webhook,
    searchKeys: ["hook", "lifecycle", "webhook", "钩子"]
  },
  {
    id: "gateways",
    group: "integrations",
    kind: "app-config",
    labelEn: "Gateways",
    labelZh: "消息网关",
    descriptionEn: "QQ, Weixin credentials and listen addresses",
    descriptionZh: "QQ、微信凭据与监听地址",
    icon: Cable,
    searchKeys: ["gateway", "qq", "weixin", "微信", "网关"]
  },
  {
    id: "git",
    group: "workspace",
    kind: "app-config",
    labelEn: "Git",
    labelZh: "Git",
    descriptionEn: "Repositories, commits, remotes, and safety",
    descriptionZh: "仓库、提交、远端和安全确认",
    icon: GitBranch,
    searchKeys: ["git", "scm", "commit", "remote", "仓库", "提交"]
  },
  {
    id: "memory",
    group: "operations",
    kind: "operations",
    labelEn: "Memory",
    labelZh: "记忆",
    descriptionEn: "Facts, events, and reset controls",
    descriptionZh: "长期事实、往事和清空",
    icon: Brain,
    searchKeys: ["memory", "fact", "event", "记忆", "事实"]
  },
  {
    id: "usage",
    group: "operations",
    kind: "analytics",
    labelEn: "Usage",
    labelZh: "用量",
    descriptionEn: "Token trends, providers, models, and request logs",
    descriptionZh: "Token 趋势、供应商、模型与请求日志",
    icon: BarChart3,
    searchKeys: ["usage", "token", "stats", "log", "用量", "统计"]
  },
  {
    id: "advanced",
    group: "advanced",
    kind: "app-config",
    labelEn: "Advanced JSON",
    labelZh: "高级 JSON",
    descriptionEn: "Complete AppConfig JSON",
    descriptionZh: "完整 AppConfig JSON",
    icon: Braces,
    searchKeys: ["json", "advanced", "appconfig", "高级"]
  }
];

/**
 * 解析路由 section 参数；未知值回退默认 section。
 *
 * @param value 路由参数
 * @returns 合法 SettingsSectionId
 */
export function resolveSettingsSectionId(value: string | undefined | null): SettingsSectionId {
  if (!value) return DEFAULT_SETTINGS_SECTION;
  const match = SETTINGS_SECTIONS.find((item) => item.id === value);
  return match?.id ?? DEFAULT_SETTINGS_SECTION;
}

/**
 * 按 id 查找 section 元数据。
 *
 * @param id section 标识
 * @returns 元数据；不存在时 undefined
 */
export function getSettingsSection(id: SettingsSectionId): SettingsSectionMeta | undefined {
  return SETTINGS_SECTIONS.find((item) => item.id === id);
}

/**
 * 判断顶栏是否应展示全局 AppConfig 保存控件。
 *
 * @param kind 当前 section 面类型
 * @returns 需要全局 Save 时 true
 */
export function showsGlobalAppConfigSave(kind: SettingsSurfaceKind): boolean {
  return kind === "app-config";
}

/**
 * 按关键字过滤 section（标签、描述、searchKeys）。
 *
 * @param query 用户输入
 * @param locale 当前语言（仅影响匹配标签字段优先级，关键字本身中英均可）
 * @returns 过滤后的 section 列表
 */
export function filterSettingsSections(query: string, locale: "en-US" | "zh-CN" = "en-US"): SettingsSectionMeta[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return SETTINGS_SECTIONS;
  return SETTINGS_SECTIONS.filter((item) => {
    const haystacks = [
      item.id,
      item.labelEn,
      item.labelZh,
      item.descriptionEn,
      item.descriptionZh,
      ...item.searchKeys
    ].map((value) => value.toLowerCase());
    // locale 预留：当前中英关键字一并匹配
    void locale;
    return haystacks.some((value) => value.includes(needle));
  });
}

/**
 * 将 section 列表按分组顺序归组。
 *
 * @param sections section 列表
 * @returns 分组后的结构（跳过空组）
 */
export function groupSettingsSections(
  sections: SettingsSectionMeta[]
): Array<{ group: SettingsGroupMeta; sections: SettingsSectionMeta[] }> {
  return SETTINGS_GROUPS.map((group) => ({
    group,
    sections: sections.filter((item) => item.group === group.id)
  })).filter((entry) => entry.sections.length > 0);
}

/**
 * 判断字符串是否为已知分组 id。
 *
 * @param value 候选值
 * @returns 是分组 id 时 true
 */
export function isSettingsGroupId(value: string): value is SettingsGroupId {
  return SETTINGS_GROUPS.some((group) => group.id === value);
}
