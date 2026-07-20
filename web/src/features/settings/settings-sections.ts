import { BarChart3, Bot, Brain, Braces, Cable, GitBranch, KeyRound, Palette, Plug, Server, SlidersHorizontal, Webhook } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { SettingsSectionId } from "./settings-types";

export type SettingsSectionMeta = {
  id: SettingsSectionId;
  labelEn: string;
  labelZh: string;
  descriptionEn: string;
  descriptionZh: string;
  icon: LucideIcon;
};

/**
 * 设置页导航分类配置。
 *
 * 新增分类时在此数组追加一项，并在 settings-types.ts 的
 * SettingsSectionId 联合类型中补充对应标识即可。
 */
export const SETTINGS_SECTIONS: SettingsSectionMeta[] = [
  { id: "providers", labelEn: "Providers and models", labelZh: "供应商与模型", descriptionEn: "Endpoints, credentials, and model lists", descriptionZh: "接口、凭据和模型列表", icon: KeyRound },
  { id: "agents", labelEn: "Agent profiles", labelZh: "Agent 配置", descriptionEn: "Prompts, tools, and skill exposure", descriptionZh: "系统提示词、工具与技能暴露", icon: Bot },
  { id: "plugins", labelEn: "Plugins", labelZh: "插件配置", descriptionEn: "Search, vision, knowledge, and memory", descriptionZh: "搜索、视觉、知识库和记忆", icon: Plug },
  { id: "runtime", labelEn: "Runtime", labelZh: "运行参数", descriptionEn: "Tools, skills, display, and context", descriptionZh: "工具、技能、显示和上下文", icon: SlidersHorizontal },
  { id: "git", labelEn: "Git and Source Control", labelZh: "Git 与源代码管理", descriptionEn: "Repositories, commits, remotes, and safety", descriptionZh: "仓库、提交、远端和安全确认", icon: GitBranch },
  { id: "appearance", labelEn: "Appearance", labelZh: "主题与配色", descriptionEn: "Language, theme, and colors", descriptionZh: "界面语言、主题和颜色方案", icon: Palette },
  { id: "gateways", labelEn: "Gateways", labelZh: "消息网关", descriptionEn: "QQ, Weixin, and runtime status", descriptionZh: "QQ、微信和运行状态", icon: Cable },
  { id: "memory", labelEn: "Memory", labelZh: "记忆管理", descriptionEn: "Facts, events, and reset controls", descriptionZh: "长期事实、往事和清空", icon: Brain },
  { id: "hooks", labelEn: "Hooks", labelZh: "Hooks", descriptionEn: "Lifecycle shell and HTTP actions", descriptionZh: "生命周期 shell 与 HTTP 动作", icon: Webhook },
  { id: "mcp", labelEn: "MCP", labelZh: "MCP", descriptionEn: "External Model Context Protocol servers", descriptionZh: "外部 MCP 工具服务", icon: Server },
  { id: "usage", labelEn: "Usage stats", labelZh: "用量统计", descriptionEn: "Token trends, providers, models, and request logs", descriptionZh: "Token 趋势、供应商、模型与请求日志", icon: BarChart3 },
  { id: "advanced", labelEn: "Advanced", labelZh: "高级配置", descriptionEn: "Complete AppConfig JSON", descriptionZh: "完整 AppConfig JSON", icon: Braces }
];
