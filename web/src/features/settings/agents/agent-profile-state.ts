import type { AgentProfileConfig, SubagentProfileConfig } from "../../../api/contracts";
import { buildDefaultAgent, DEFAULT_AGENT_ID } from "../../agents/agent-options";
import type { AgentProfile } from "../../agents/agent-types";
import type { AgentOptions } from "./agents-types";
import { text, type Locale } from "../../i18n/locale";

export const BUILTIN_AGENT_PROFILES: AgentProfileConfig[] = [
  {
    id: "cli",
    name: "CLI 助手",
    description: "人格化终端助手：工具全量开放，适合日常排障与对话",
    thinking_level: "auto",
    register_to_main: false,
    load_instruction_files: true
  },
  {
    id: "general",
    name: "代码 Agent",
    description: "适合实现、测试、文档和常规工程任务；工具面向长程编程",
    thinking_level: "auto",
    register_to_main: true,
    load_instruction_files: true
  },
  {
    id: "explore",
    name: "探索 Agent",
    description: "适合只读检索、代码定位和资料探索；返回证据与路径",
    enabled_tools: ["check_os_info", "read_file", "glob", "grep", "web_search", "web_fetch"],
    thinking_level: "auto",
    register_to_main: true,
    load_instruction_files: true
  },
  {
    id: "plan",
    name: "Plan Agent",
    description: "只读调研与方案规划，不改系统状态",
    enabled_tools: [
      "check_os_info",
      "read_file",
      "glob",
      "grep",
      "web_search",
      "web_fetch",
      "ask_question"
    ],
    thinking_level: "auto",
    register_to_main: true,
    load_instruction_files: true
  },
  {
    id: "gateway",
    name: "网关 Agent",
    description: "适合 QQ/微信等即时通讯网关：短回复、排障与查询",
    enabled_tools: [
      "check_os_info",
      "read_file",
      "glob",
      "grep",
      "run_command",
      "web_search",
      "web_fetch",
      "query_weather",
      "remember_fact",
      "recall_memories",
      "cron",
      "send_channel_message"
    ],
    thinking_level: "auto",
    register_to_main: false,
    load_instruction_files: false
  }
];

/**
 * 将配置文件中的主 Agent 档案转换成编辑器可直接使用的完整档案。
 *
 * @param profile 配置文件中的主 Agent 档案
 * @param defaults 可选的字段默认值
 * @returns 所有编辑字段均存在的主 Agent 档案
 */
export function normalizeAgentProfile(
  profile: AgentProfileConfig,
  defaults: Partial<AgentProfile> = {}
): AgentProfile {
  const id = profile.id || defaults.id || "agent";
  return {
    id,
    name: profile.name || defaults.name || id,
    description: profile.description ?? defaults.description ?? "",
    system_prompt: profile.system_prompt ?? defaults.system_prompt ?? "",
    enabled_tools: copyList(profile.enabled_tools ?? defaults.enabled_tools),
    skills_full: copyList(profile.skills_full ?? defaults.skills_full),
    skills_named: copyList(profile.skills_named ?? defaults.skills_named),
    provider_id: profile.provider_id ?? defaults.provider_id ?? "",
    model: profile.model ?? defaults.model ?? "",
    thinking_level: profile.thinking_level ?? defaults.thinking_level ?? "auto",
    register_to_main: profile.register_to_main ?? defaults.register_to_main ?? false,
    load_instruction_files: profile.load_instruction_files ?? defaults.load_instruction_files ?? true
  };
}

/**
 * 构造设置界面可见的主 Agent 列表，缺少持久化默认档案时补充虚拟默认项。
 *
 * @param profiles 已存储的主 Agent 档案
 * @param options 当前可用的工具与 Skill 选项
 * @param legacyProfiles 旧版子 Agent 档案
 * @returns 可供界面展示的完整主 Agent 档案列表
 */
export function buildVisibleAgentProfiles(
  profiles: readonly AgentProfileConfig[] | undefined,
  options: AgentOptions,
  legacyProfiles: readonly SubagentProfileConfig[] | undefined = undefined,
  locale: Locale = "zh-CN"
): AgentProfile[] {
  const configured = profiles ?? [];
  const migrated = (legacyProfiles ?? [])
    .filter((legacy) => !configured.some((profile) => profile.id === legacy.id))
    .map((legacy): AgentProfileConfig => ({
      ...legacy,
      register_to_main: legacy.exposed !== false
    }));
  const stored = [...configured, ...migrated];
  const normalized = stored.map((profile) => normalizeAgentProfile(profile));
  const builtins = localizedBuiltinProfiles(locale).map((builtin) => {
    const override = stored.find((profile) => profile.id === builtin.id);
    const defaults = builtin.id === "cli"
      ? {
          ...builtin,
          enabled_tools: options.tools.map((tool) => tool.name),
          skills_full: options.skills.map((skill) => skill.name)
        }
      : builtin.id === "general"
        ? {
            ...builtin,
            // 代码 Agent 若无覆盖则沿用后端白名单；UI 编辑时再细化
            enabled_tools: builtin.enabled_tools ?? options.tools.map((tool) => tool.name),
            skills_full: options.skills.map((skill) => skill.name)
          }
        : builtin;
    return normalizeAgentProfile(override ?? defaults, normalizeAgentProfile(defaults));
  });
  const custom = normalized.filter((profile) => !BUILTIN_AGENT_PROFILES.some((builtin) => builtin.id === profile.id));
  const visible = [...builtins, ...custom];
  if (!visible.some((profile) => profile.id === DEFAULT_AGENT_ID)) {
    visible.unshift(buildDefaultAgent(options, locale));
  }
  return visible;
}

/**
 * 创建一个启用全部当前选项的唯一自定义主 Agent 档案。
 *
 * @param profiles 已存储的主 Agent 档案
 * @param options 当前可用的工具与 Skill 选项
 * @param overrides 新档案需要覆盖的字段
 * @returns 尚未写入配置的全新主 Agent 档案
 */
export function createUniqueAgentProfile(
  profiles: readonly AgentProfileConfig[],
  options: AgentOptions,
  overrides: Partial<AgentProfile> = {},
  locale: Locale = "zh-CN"
): AgentProfile {
  const id = nextNumericId(profiles.map((profile) => profile.id), "agent");
  const number = id.slice("agent-".length);
  const { id: _ignoredId, ...fields } = overrides;
  return normalizeAgentProfile({
    id,
    name: text(locale, `New Agent ${number}`, `新 Agent ${number}`),
    description: "",
    system_prompt: "",
    enabled_tools: options.tools.map((tool) => tool.name),
    skills_full: options.skills.map((skill) => skill.name),
    skills_named: [],
    provider_id: "",
    model: "",
    thinking_level: "auto",
    register_to_main: false,
    ...fields
  });
}

/**
 * 返回指定语言的内置 Agent 档案。
 *
 * @param locale 当前界面语言
 * @returns 带本地化名称和说明的内置档案
 */
function localizedBuiltinProfiles(locale: Locale): AgentProfileConfig[] {
  return BUILTIN_AGENT_PROFILES.map((profile) => {
    if (profile.id === "general") return {
      ...profile,
      name: text(locale, "Coding Agent", "代码 Agent"),
      description: text(locale, "Suitable for implementation, testing, documentation, and general engineering tasks", "适合实现、测试、文档和常规工程任务")
    };
    if (profile.id === "explore") return {
      ...profile,
      name: text(locale, "Explore Agent", "探索 Agent"),
      description: text(locale, "Suitable for read-only search, code navigation, and research", "适合只读检索、代码定位和资料探索")
    };
    return {
      ...profile,
      name: text(locale, "Gateway Agent", "网关 Agent"),
      description: text(locale, "Suitable for QQ, WeChat, and other messaging gateways: concise replies, troubleshooting, and queries", "适合 QQ/微信等即时通讯网关：短回复、排障与查询")
    };
  });
}

/**
 * 更新指定主 Agent 档案，并保持档案标识不可变。
 *
 * @param profiles 已存储的主 Agent 档案
 * @param id 待更新档案的标识
 * @param patch 需要覆盖的可选字段
 * @returns 更新后的新数组
 */
export function updateAgentProfile(
  profiles: readonly AgentProfileConfig[],
  id: string,
  patch: Partial<AgentProfileConfig>
): AgentProfileConfig[] {
  const { id: _ignoredId, ...fields } = patch;
  return profiles.map((profile) => profile.id === id ? { ...profile, ...fields, id } : { ...profile });
}

/**
 * 删除指定主 Agent 档案。
 *
 * @param profiles 已存储的主 Agent 档案
 * @param id 待删除档案的标识
 * @returns 删除后的新数组
 */
export function removeAgentProfile(
  profiles: readonly AgentProfileConfig[],
  id: string
): AgentProfileConfig[] {
  return profiles.filter((profile) => profile.id !== id).map((profile) => ({ ...profile }));
}

/**
 * 复制可选字符串数组，避免状态函数修改调用方持有的数组。
 *
 * @param values 待复制的可选字符串数组
 * @returns 独立的字符串数组
 */
function copyList(values: readonly string[] | undefined): string[] {
  return Array.isArray(values) ? [...values] : [];
}

/**
 * 根据前缀找到第一个未使用的数字标识。
 *
 * @param ids 已使用的标识
 * @param prefix 新标识的前缀
 * @returns 第一个未占用的数字标识
 */
function nextNumericId(ids: readonly string[], prefix: string): string {
  const used = new Set(ids);
  let suffix = 1;
  while (used.has(`${prefix}-${suffix}`)) suffix += 1;
  return `${prefix}-${suffix}`;
}
