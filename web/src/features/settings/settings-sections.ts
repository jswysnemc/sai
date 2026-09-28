import {
  BarChart3,
  Bot,
  Brain,
  Braces,
  Cable,
  Database,
  DiamondCheck,
  FileText,
  GitBranch,
  Globe,
  ImagePlus,
  KeyRound,
  Palette,
  PlugZap,
  Server,
  SlidersHorizontal,
  Sparkles,
  Wrench,
  Webhook
} from "../../shared/ui/icons";
import type { SettingsGroupMeta, SettingsSectionMeta } from "./settings-types";

/**
 * 侧栏分组顺序与文案，与 ZCode 的基础 / 智能体能力 / 数据与统计对齐。
 */
export const SETTINGS_GROUPS: SettingsGroupMeta[] = [
  { id: "basics", labelEn: "Basics", labelZh: "基础" },
  { id: "agentCapabilities", labelEn: "Agent capabilities", labelZh: "智能体能力" },
  { id: "dataAndStats", labelEn: "Data and stats", labelZh: "数据与统计" }
];

/**
 * 设置页 section 注册表。
 *
 * 新增 section：
 * 1. 在 SettingsSectionId 联合类型中补充 id
 * 2. 在本数组追加元数据
 * 3. 在 SettingsSectionBody 与 settings-section-routing 中挂载组件
 */
export const SETTINGS_SECTIONS: SettingsSectionMeta[] = [
  {
    id: "providers",
    group: "basics",
    appConfig: "required",
    layout: "wide",
    configKeys: ["providers", "active_provider"],
    labelEn: "LLM providers",
    labelZh: "LLM 供应商",
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
    id: "image-models",
    group: "basics",
    appConfig: "required",
    layout: "wide",
    configKeys: ["model_endpoints"],
    labelEn: "Image models",
    labelZh: "生图模型",
    descriptionEn: "Image model request endpoints and API keys",
    descriptionZh: "生图模型的独立请求地址和 API Key",
    icon: ImagePlus,
    searchKeys: ["image", "generation", "model", "endpoint", "生图", "图片", "密钥"]
  },
  {
    id: "appearance",
    group: "basics",
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
    id: "runtime",
    group: "basics",
    appConfig: "required",
    configKeys: ["agent", "session", "retry", "permission", "notification", "terminal", "input", "context", "tools", "display", "mesh", "debug", "memory"],
    labelEn: "Runtime",
    labelZh: "运行时",
    descriptionEn: "Sessions, permissions, terminal, and display",
    descriptionZh: "会话、权限、终端与显示",
    icon: SlidersHorizontal,
    searchKeys: ["runtime", "session", "model", "thinking", "permission", "notification", "terminal", "context", "display", "tools", "debug", "api", "mesh", "cross_session", "retry", "backoff", "会话", "模型", "思考", "权限", "通知", "终端", "上下文", "压缩比例", "预留", "调试", "网格", "跨会话", "重试", "退避", "失败"],
    subviews: [
      { id: "execution", labelEn: "Execution and sessions", labelZh: "执行与会话" },
      { id: "environment", labelEn: "Environment and permissions", labelZh: "环境与权限" },
      { id: "tools", labelEn: "Tools and context", labelZh: "工具与上下文" }
    ],
    legacySubviews: {
      engine: "execution",
      permissions: "environment",
      terminal: "environment",
      context: "tools"
    }
  },
  {
    id: "prompts",
    group: "basics",
    appConfig: "required",
    layout: "wide",
    configKeys: ["prompt"],
    labelEn: "Internal prompts",
    labelZh: "内部提示词",
    descriptionEn: "Commit messages, session titles, and context compaction",
    descriptionZh: "提交说明、会话标题与上下文压缩",
    icon: FileText,
    searchKeys: ["prompt", "template", "commit", "title", "compaction", "variable", "提示词", "模板", "提交", "标题", "压缩", "变量"]
  },
  {
    id: "git",
    group: "basics",
    appConfig: "required",
    configKeys: ["git", "scm"],
    labelEn: "Git",
    labelZh: "Git",
    descriptionEn: "Repositories, commits, remotes, and safety",
    descriptionZh: "仓库、提交、远端和安全确认",
    icon: GitBranch,
    searchKeys: ["git", "scm", "commit", "remote", "仓库", "提交"]
  },
  {
    id: "ssh",
    group: "basics",
    appConfig: "optional",
    layout: "wide",
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
    id: "agents",
    group: "basics",
    appConfig: "required",
    layout: "wide",
    configKeys: ["agents", "default_agent", "tui_agent", "cli_agent", "gateway_agent", "subagent"],
    labelEn: "Agent profiles",
    labelZh: "Agent 配置",
    descriptionEn: "Prompts, tools, and skill exposure",
    descriptionZh: "系统提示词、工具与技能暴露",
    icon: Bot,
    searchKeys: ["agent", "prompt", "tool", "skill", "权限"]
  },
  {
    id: "cli-tools",
    group: "agentCapabilities",
    appConfig: "required",
    layout: "wide",
    configKeys: ["plugins"],
    labelEn: "CLI assistant tools",
    labelZh: "CLI 助手工具",
    descriptionEn: "Optional tools exposed to CLI assistants",
    descriptionZh: "配置 CLI 助手可使用的可选工具",
    icon: Wrench,
    searchKeys: ["cli", "assistant", "tool", "optional", "plugin", "助手", "工具", "可选工具", "插件"]
  },
  {
    id: "web-search",
    group: "agentCapabilities",
    appConfig: "required",
    layout: "wide",
    configKeys: ["plugins.web"],
    labelEn: "Web search",
    labelZh: "网页搜索",
    descriptionEn: "Built-in search routing, endpoints, and credentials",
    descriptionZh: "内置搜索路由、供应商地址与凭据",
    icon: Globe,
    searchKeys: ["web-search", "search", "tinyfish", "tavily", "firecrawl", "anysearch", "searxng", "duckduckgo", "网页", "搜索", "联网"]
  },
  {
    id: "jev",
    group: "agentCapabilities",
    appConfig: "required",
    configKeys: ["jev"],
    labelEn: "Jev",
    labelZh: "Jev",
    descriptionEn: "Tool routing, permission audit, and connections",
    descriptionZh: "工具暴露决策、权限自动审核与接入",
    icon: DiamondCheck,
    searchKeys: ["jev", "typesafe", "routing", "audit", "capability", "endpoint", "决策", "暴露", "审核", "密钥"]
  },
  {
    id: "skills",
    group: "agentCapabilities",
    appConfig: "optional",
    layout: "wide",
    configKeys: ["skills"],
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
    group: "agentCapabilities",
    appConfig: "none",
    layout: "wide",
    saveHintEn: "Saves in section",
    saveHintZh: "在本节内保存",
    labelEn: "MCP",
    labelZh: "MCP",
    descriptionEn: "External Model Context Protocol servers",
    descriptionZh: "外部 MCP 工具服务",
    icon: PlugZap,
    searchKeys: ["mcp", "stdio", "sse", "server", "工具服务"]
  },
  {
    id: "hooks",
    group: "agentCapabilities",
    appConfig: "required",
    layout: "wide",
    configKeys: ["hooks"],
    labelEn: "Hooks",
    labelZh: "Hooks",
    descriptionEn: "Lifecycle shell and HTTP actions",
    descriptionZh: "生命周期 shell 与 HTTP 动作",
    icon: Webhook,
    searchKeys: ["hook", "lifecycle", "webhook", "钩子"]
  },
  {
    id: "gateways",
    group: "agentCapabilities",
    appConfig: "required",
    configKeys: ["gateways"],
    labelEn: "Gateways",
    labelZh: "消息网关",
    descriptionEn: "QQ, Weixin credentials and listen addresses",
    descriptionZh: "QQ、微信凭据与监听地址",
    icon: Cable,
    searchKeys: ["gateway", "qq", "weixin", "微信", "网关"]
  },
  {
    id: "memory",
    group: "agentCapabilities",
    appConfig: "optional",
    layout: "wide",
    configKeys: ["plugins.memory"],
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
    group: "dataAndStats",
    appConfig: "none",
    layout: "wide",
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
    group: "dataAndStats",
    appConfig: "none",
    layout: "wide",
    saveHintEn: "Read only",
    saveHintZh: "只读",
    labelEn: "Usage",
    labelZh: "用量",
    descriptionEn: "Token trends, top-consuming sessions, providers, models, and request logs",
    descriptionZh: "Token 趋势、高消耗会话、供应商、模型与请求日志",
    icon: BarChart3,
    searchKeys: ["usage", "token", "stats", "log", "ranking", "用量", "统计", "高消耗", "排行"],
    subviews: [
      { id: "overview", labelEn: "Overview", labelZh: "总览" },
      { id: "breakdown", labelEn: "Breakdown", labelZh: "多维对比" },
      { id: "logs", labelEn: "Request logs", labelZh: "请求日志" }
    ],
    legacySubviews: {
      providers: "breakdown",
      models: "breakdown",
      sessions: "breakdown"
    }
  },
  {
    id: "advanced",
    group: "dataAndStats",
    appConfig: "required",
    layout: "wide",
    labelEn: "Advanced JSON",
    labelZh: "高级 JSON",
    descriptionEn: "Complete AppConfig JSON",
    descriptionZh: "完整 AppConfig JSON",
    icon: Braces,
    searchKeys: ["json", "advanced", "appconfig", "高级"]
  }
];
