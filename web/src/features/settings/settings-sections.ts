import { Bot, Brain, Braces, Cable, KeyRound, Palette, Plug, SlidersHorizontal, Webhook } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { SettingsSectionId } from "./settings-types";

export type SettingsSectionMeta = {
  id: SettingsSectionId;
  label: string;
  description: string;
  icon: LucideIcon;
};

/**
 * 设置页导航分类配置。
 *
 * 新增分类时在此数组追加一项，并在 settings-types.ts 的
 * SettingsSectionId 联合类型中补充对应标识即可。
 */
export const SETTINGS_SECTIONS: SettingsSectionMeta[] = [
  { id: "providers", label: "供应商与模型", description: "接口、凭据和模型列表", icon: KeyRound },
  { id: "agents", label: "Agent 配置", description: "系统提示词、工具与技能暴露", icon: Bot },
  { id: "plugins", label: "插件配置", description: "搜索、视觉、知识库和记忆", icon: Plug },
  { id: "runtime", label: "运行参数", description: "工具、技能、显示和上下文", icon: SlidersHorizontal },
  { id: "appearance", label: "主题与配色", description: "界面主题和颜色方案", icon: Palette },
  { id: "gateways", label: "消息网关", description: "QQ、微信和运行状态", icon: Cable },
  { id: "memory", label: "记忆管理", description: "长期事实、往事和清空", icon: Brain },
  { id: "hooks", label: "Hooks 与 MCP", description: "生命周期钩子和外部 MCP 工具", icon: Webhook },
  { id: "advanced", label: "高级配置", description: "完整 AppConfig JSON", icon: Braces }
];
