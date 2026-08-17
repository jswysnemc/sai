import {
  BarChart3,
  Bot,
  Brain,
  Braces,
  Cable,
  Database,
  FileText,
  GitBranch,
  KeyRound,
  Palette,
  Search,
  Server,
  SlidersHorizontal,
  Sparkles,
  Wrench,
  Webhook
} from "lucide-react";
import type {
  SettingsAppConfigUse,
  SettingsGroupId,
  SettingsGroupMeta,
  SettingsSectionId,
  SettingsSectionMeta
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
    appConfig: "required",
    labelEn: "Providers and models",
    labelZh: "供应商与模型",
    descriptionEn: "Endpoints, credentials, and model lists",
    descriptionZh: "接口、凭据和模型列表",
    icon: KeyRound,
    searchKeys: ["provider", "model", "api_key", "base_url", "供应商", "模型", "凭据"],
    subviews: [
      { id: "connection", labelEn: "Connection", labelZh: "连接" },
      { id: "models", labelEn: "Models", labelZh: "模型" },
      { id: "behavior", labelEn: "Behavior", labelZh: "行为" },
      { id: "advanced", labelEn: "Advanced", labelZh: "高级" }
    ]
  },
  {
    id: "agents",
    group: "general",
    appConfig: "required",
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
    appConfig: "required",
    labelEn: "Runtime",
    labelZh: "运行时",
    descriptionEn: "Sessions, permissions, notifications, terminal, and display",
    descriptionZh: "会话、权限、通知、终端与显示",
    icon: SlidersHorizontal,
    searchKeys: ["runtime", "session", "model", "thinking", "permission", "notification", "terminal", "context", "display", "tools", "debug", "api", "会话", "模型", "思考", "权限", "通知", "终端", "上下文", "压缩比例", "预留", "调试"],
    subviews: [
      { id: "engine", labelEn: "Engine", labelZh: "对话内核" },
      { id: "permissions", labelEn: "Permissions", labelZh: "权限" },
      { id: "notifications", labelEn: "Notifications", labelZh: "通知" },
      { id: "terminal", labelEn: "Terminal", labelZh: "终端" },
      { id: "context", labelEn: "Context", labelZh: "上下文" },
      { id: "tools", labelEn: "Tools and display", labelZh: "工具与显示" }
    ]
  },
  {
    id: "appearance",
    group: "general",
    appConfig: "none",
    saveHintEn: "Applies immediately",
    saveHintZh: "即时生效",
    labelEn: "Appearance",
    labelZh: "外观",
    descriptionEn: "Language, theme, colors, and Markdown rendering",
    descriptionZh: "界面语言、主题、颜色与 Markdown 渲染",
    icon: Palette,
    searchKeys: ["theme", "language", "locale", "appearance", "markdown", "table", "code", "主题", "语言", "配色", "表格", "代码块"]
  },
  {
    id: "prompts",
    group: "general",
    appConfig: "required",
    labelEn: "Internal prompts",
    labelZh: "内部提示词",
    descriptionEn: "Commit messages, session titles, and context compaction",
    descriptionZh: "提交说明、会话标题与上下文压缩",
    icon: FileText,
    searchKeys: ["prompt", "template", "commit", "title", "compaction", "variable", "提示词", "模板", "提交", "标题", "压缩", "变量"]
  },
  {
    id: "cli-tools",
    group: "integrations",
    appConfig: "required",
    labelEn: "CLI assistant tools",
    labelZh: "CLI 助手工具",
    descriptionEn: "Optional tools exposed to CLI assistants",
    descriptionZh: "配置 CLI 助手可使用的可选工具",
    icon: Wrench,
    searchKeys: ["cli", "assistant", "tool", "optional", "plugin", "助手", "工具", "可选工具", "插件"]
  },
  {
    id: "web-search",
    group: "integrations",
    appConfig: "required",
    labelEn: "Web search",
    labelZh: "Web 搜索",
    descriptionEn: "Provider credentials, endpoints, and search behavior",
    descriptionZh: "搜索供应商、凭据、服务地址与检索行为",
    icon: Search,
    searchKeys: ["web", "search", "provider", "tinyfish", "tavily", "firecrawl", "anysearch", "searxng", "网页", "搜索", "供应商"]
  },
  {
    id: "skills",
    group: "integrations",
    appConfig: "optional",
    saveHintEn: "Actions in section",
    saveHintZh: "操作在本节内完成",
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
    appConfig: "none",
    saveHintEn: "Saves in section",
    saveHintZh: "在本节内保存",
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
    appConfig: "required",
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
    appConfig: "required",
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
    appConfig: "required",
    labelEn: "Git",
    labelZh: "Git",
    descriptionEn: "Repositories, commits, remotes, and safety",
    descriptionZh: "仓库、提交、远端和安全确认",
    icon: GitBranch,
    searchKeys: ["git", "scm", "commit", "remote", "仓库", "提交"]
  },
  {
    id: "ssh",
    group: "workspace",
    appConfig: "optional",
    saveHintEn: "Actions in section",
    saveHintZh: "操作在本节内完成",
    labelEn: "SSH",
    labelZh: "SSH",
    descriptionEn: "Remote hosts for terminal sessions",
    descriptionZh: "终端会话可用的远程主机",
    icon: Server,
    searchKeys: ["ssh", "remote", "host", "terminal", "远程", "主机", "终端"]
  },
  {
    id: "memory",
    group: "operations",
    appConfig: "optional",
    saveHintEn: "Actions in section",
    saveHintZh: "操作在本节内完成",
    labelEn: "Memory",
    labelZh: "记忆",
    descriptionEn: "Memory files, scopes, and evicted context",
    descriptionZh: "记忆文件、作用域与逐出上下文",
    icon: Brain,
    searchKeys: ["memory", "note", "fact", "记忆", "笔记"]
  },
  {
    id: "session-data",
    group: "operations",
    appConfig: "none",
    saveHintEn: "Actions in section",
    saveHintZh: "操作在本节内完成",
    labelEn: "Session data",
    labelZh: "会话数据",
    descriptionEn: "Inspect, clear, and delete workspace sessions",
    descriptionZh: "查看、清空和删除工作区会话",
    icon: Database,
    searchKeys: ["session", "data", "storage", "clear", "delete", "会话", "数据", "清空", "删除"]
  },
  {
    id: "usage",
    group: "operations",
    appConfig: "none",
    saveHintEn: "Read only",
    saveHintZh: "只读",
    labelEn: "Usage",
    labelZh: "用量",
    descriptionEn: "Token trends, providers, models, and request logs",
    descriptionZh: "Token 趋势、供应商、模型与请求日志",
    icon: BarChart3,
    searchKeys: ["usage", "token", "stats", "log", "用量", "统计"],
    subviews: [
      { id: "overview", labelEn: "Overview", labelZh: "总览" },
      { id: "providers", labelEn: "By provider", labelZh: "按供应商" },
      { id: "models", labelEn: "By model", labelZh: "按模型" },
      { id: "logs", labelEn: "Request logs", labelZh: "请求日志" }
    ]
  },
  {
    id: "advanced",
    group: "advanced",
    appConfig: "required",
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
  // 1. 兼容旧版插件设置地址，并统一迁移到 CLI 助手工具语义
  if (value === "plugins") return "cli-tools";
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
 * 解析二级子页路由段。
 *
 * 无子页的分区始终返回 undefined；有子页的分区在段非法或缺失时
 * 回落到首个子页，保证 URL 总能归一到显式子页。
 *
 * @param meta 分区元数据
 * @param value 路由中的子页段
 * @returns 合法子页 id；分区无子页时 undefined
 */
export function resolveSettingsSubview(
  meta: SettingsSectionMeta | undefined,
  value: string | undefined | null
): string | undefined {
  const subviews = meta?.subviews;
  if (!subviews || subviews.length === 0) return undefined;
  return subviews.find((item) => item.id === value)?.id ?? subviews[0].id;
}

/**
 * 判断顶栏是否应展示全局 AppConfig 保存控件。
 *
 * required 面常驻保存；optional 面只在有待保存修改时露出，
 * 平时与 none 面一样显示分区自己的保存提示。
 *
 * @param use 分区对 AppConfig 的参与方式
 * @param dirty 全局草稿是否有待保存修改
 * @returns 需要全局 Save 时 true
 */
export function showsAppConfigSave(use: SettingsAppConfigUse, dirty: boolean): boolean {
  return use === "required" || (use === "optional" && dirty);
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
