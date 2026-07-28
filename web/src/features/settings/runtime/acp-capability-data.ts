import type { AgentEngineKind, EngineStatusResponse } from "../../../api/contracts";

export type AcpCapabilities = NonNullable<EngineStatusResponse["acp_capabilities"]>;
export type AcpCapabilityId = keyof AcpCapabilities;
export type AcpConnectionState = "loading" | "disconnected" | "connected" | "partial" | "error";

export type AcpCapabilityItem = {
  id: AcpCapabilityId;
  label: { en: string; zh: string };
  description: { en: string; zh: string };
};

export type AcpCapabilityGroups = {
  standard: AcpCapabilityItem[];
  sai: AcpCapabilityItem[];
  codexNative: AcpCapabilityItem[];
  unsupported: AcpCapabilityItem[];
};

export type AcpCommand = {
  name: string;
  description: string;
};

const standardCapabilities: readonly AcpCapabilityItem[] = [
  capability("load_session", "Load session", "加载会话", "Restore a persisted ACP session", "恢复已持久化的 ACP 会话"),
  capability("list_sessions", "List sessions", "列出会话", "List sessions managed by the agent", "列出由内核管理的会话"),
  capability("delete_session", "Delete session", "删除会话", "Delete a persisted agent session", "删除已持久化的内核会话"),
  capability("resume_session", "Resume session", "继续会话", "Resume an interrupted agent session", "继续被中断的内核会话"),
  capability("close_session", "Close session", "关闭会话", "Close an active agent session", "关闭活动内核会话"),
  capability("additional_directories", "Additional directories", "附加目录", "Share extra workspace directories", "共享额外工作区目录"),
  capability("mcp_http", "MCP HTTP", "MCP HTTP", "Connect HTTP MCP servers", "连接 HTTP MCP 服务"),
  capability("mcp_sse", "MCP SSE", "MCP SSE", "Connect SSE MCP servers", "连接 SSE MCP 服务"),
  capability("prompt_image", "Image input", "图片输入", "Accept images in ACP prompts", "在 ACP 提示中接收图片"),
  capability("prompt_audio", "Audio input", "音频输入", "Accept audio in ACP prompts", "在 ACP 提示中接收音频"),
  capability("embedded_context", "Embedded context", "嵌入上下文", "Accept structured resource context", "接收结构化资源上下文"),
  capability("logout", "Logout", "退出认证", "End the current ACP authentication", "结束当前 ACP 认证")
];

const saiCapabilities: readonly AcpCapabilityItem[] = [
  capability("sai_context_compaction", "Context compaction", "上下文压缩", "Run compaction through the external engine", "通过外部内核执行上下文压缩"),
  capability("sai_memory", "Memory injection", "记忆注入", "Inject related Sai long-term memory", "注入相关的 Sai 长期记忆"),
  capability("sai_goal_continuation", "Goal continuation", "活动目标延续", "Continue active Sai goals across turns", "跨轮次继续活动 Sai 目标"),
  capability("sai_subagents", "Subagents", "子智能体", "Expose external subagent activity to Sai", "向 Sai 暴露外部子智能体活动")
];

const nativeEquivalentCapabilityIds: Readonly<Record<string, AcpCapabilityId>> = {
  context_compaction: "sai_context_compaction",
  subagents: "sai_subagents"
};

/**
 * 构造一项双语能力定义。
 *
 * @param id 后端能力字段
 * @param en 英文名称
 * @param zh 中文名称
 * @param descriptionEn 英文说明
 * @param descriptionZh 中文说明
 * @returns 能力展示定义
 */
function capability(
  id: AcpCapabilityId,
  en: string,
  zh: string,
  descriptionEn: string,
  descriptionZh: string
): AcpCapabilityItem {
  return {
    id,
    label: { en, zh },
    description: { en: descriptionEn, zh: descriptionZh }
  };
}

/**
 * 按协议来源和支持状态整理 ACP 能力。
 *
 * @param engine 当前外部内核
 * @param capabilities 最近一次握手能力
 * @param nativeEquivalents agent 公布的原生等价能力
 * @returns 标准 ACP、Sai 集成、Codex 原生等价与未支持能力
 */
export function groupAcpCapabilities(
  engine: AgentEngineKind,
  capabilities: AcpCapabilities | null | undefined,
  nativeEquivalents?: unknown
): AcpCapabilityGroups {
  const groups: AcpCapabilityGroups = {
    standard: [],
    sai: [],
    codexNative: [],
    unsupported: []
  };
  if (!capabilities) return groups;
  const nativeIds = parseNativeEquivalentIds(engine, nativeEquivalents);

  // 1. 标准协议能力按握手结果分别进入支持或未支持分组
  for (const item of standardCapabilities) {
    (capabilities[item.id] ? groups.standard : groups.unsupported).push(item);
  }
  // 2. Codex 的压缩与子智能体来自原生等价实现，其余能力属于 Sai 宿主集成
  for (const item of saiCapabilities) {
    if (!capabilities[item.id]) {
      groups.unsupported.push(item);
    } else if (nativeIds.has(item.id)) {
      groups.codexNative.push(item);
    } else {
      groups.sai.push(item);
    }
  }
  return groups;
}

/**
 * 解析当前内核明确公布的原生等价能力。
 *
 * @param engine 当前外部内核
 * @param input `_sai.native_equivalents` 原始值
 * @returns 对应的 Sai 能力字段集合
 */
function parseNativeEquivalentIds(engine: AgentEngineKind, input: unknown): Set<AcpCapabilityId> {
  const ids = new Set<AcpCapabilityId>();
  if (!input || typeof input !== "object" || Array.isArray(input)) return ids;
  for (const [name, provider] of Object.entries(input as Record<string, unknown>)) {
    const id = nativeEquivalentCapabilityIds[name];
    if (id && provider === engine) ids.add(id);
  }
  return ids;
}

/**
 * 解析设置页应展示的 ACP 连接状态。
 *
 * @param engine 当前草稿选择的内核
 * @param status 服务端内核状态
 * @param loading 查询是否正在首次加载
 * @param error 查询错误
 * @returns 加载中、尚未连接、已连接、部分能力或查询失败
 */
export function resolveAcpConnectionState(
  engine: AgentEngineKind,
  status: EngineStatusResponse | undefined,
  loading: boolean,
  error: unknown
): AcpConnectionState {
  if (error) return "error";
  if (loading && !status) return "loading";
  if (!status || status.engine !== engine || !status.acp_runtime) return "disconnected";
  if (status.acp_runtime.connected === false) return "disconnected";
  const capabilities = status.acp_runtime.capabilities ?? status.acp_capabilities;
  if (!capabilities) return "partial";
  const integrations = [
    capabilities.sai_context_compaction,
    capabilities.sai_memory,
    capabilities.sai_goal_continuation,
    capabilities.sai_subagents
  ];
  return integrations.every(Boolean) ? "connected" : "partial";
}

/**
 * 从运行状态中提取当前内核的能力集合。
 *
 * @param engine 当前草稿选择的内核
 * @param status 服务端内核状态
 * @returns 匹配内核的能力集合；未连接时返回空
 */
export function capabilitiesForEngine(
  engine: AgentEngineKind,
  status: EngineStatusResponse | undefined
): AcpCapabilities | null {
  if (!status || status.engine !== engine || !status.acp_runtime) return null;
  return status.acp_runtime.capabilities ?? status.acp_capabilities ?? null;
}

/**
 * 解析 agent 公布的斜杠命令。
 *
 * @param input ACP availableCommands 原始值
 * @returns 去重并补齐斜杠前缀的命令列表
 */
export function parseAcpCommands(input: unknown): AcpCommand[] {
  if (!Array.isArray(input)) return [];
  const commands: AcpCommand[] = [];
  const seen = new Set<string>();
  for (const candidate of input) {
    if (!candidate || typeof candidate !== "object") continue;
    const value = candidate as Record<string, unknown>;
    if (typeof value.name !== "string" || !value.name.trim()) continue;
    const bareName = value.name.trim().replace(/^\/+/, "");
    if (!bareName) continue;
    const name = `/${bareName}`;
    if (seen.has(name)) continue;
    seen.add(name);
    commands.push({
      name,
      description: typeof value.description === "string" ? value.description.trim() : ""
    });
  }
  return commands;
}
