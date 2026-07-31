import type { McpConfig, McpServerConfig } from "../../../api/contracts";

/**
 * 解析 MCP JSON 文本为配置对象。
 *
 * @param raw JSON 文本
 * @returns MCP 配置
 */
export function parseMcpJson(raw: string): McpConfig {
  const value = JSON.parse(raw) as unknown;
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("MCP configuration must be a JSON object");
  }
  const root = value as Record<string, unknown>;
  if (Array.isArray(root.servers)) {
    return value as McpConfig;
  }
  const serverMap = asRecord(root.mcpServers) ?? asRecord(root.servers);
  if (!serverMap) {
    if (root.servers !== undefined || root.mcpServers !== undefined) {
      throw new Error("mcpServers must be an object and servers must be an array or object");
    }
    return { enabled: readBoolean(root.enabled, true), servers: [] };
  }
  return {
    enabled: readBoolean(root.enabled, true),
    servers: Object.entries(serverMap).map(([mapId, server]) => normalizeMcpServer(mapId, server))
  };
}

/**
 * 将标准 MCP 服务对象转换为表单使用的内部结构。
 *
 * @param mapId 标准 mcpServers 对象键
 * @param value 服务配置
 * @returns 带稳定 id 与 transport 的服务配置
 */
function normalizeMcpServer(mapId: string, value: unknown): McpServerConfig {
  const server = asRecord(value);
  if (!server) throw new Error(`MCP server ${mapId} must be a JSON object`);
  const explicitId = readString(server.id)?.trim();
  const type = readString(server.transport) ?? readString(server.type);
  const transport = normalizeTransport(type, readString(server.url));
  const disabled = typeof server.disabled === "boolean" ? server.disabled : undefined;

  return {
    id: explicitId || mapId,
    enabled: typeof server.enabled === "boolean" ? server.enabled : !(disabled ?? false),
    transport,
    command: readString(server.command),
    args: readStringArray(server.args),
    env: readStringRecord(server.env),
    cwd: readNullableString(server.cwd),
    url: readNullableString(server.url),
    message_url: readNullableString(server.message_url ?? server.messageUrl),
    headers: readStringRecord(server.headers),
    timeout_ms: readNullableNumber(server.timeout_ms ?? server.timeoutMs)
  };
}

/**
 * 归一化标准 MCP 传输类型。
 *
 * @param type 显式 transport 或 type
 * @param url 可选远端地址
 * @returns 内部传输类型
 */
function normalizeTransport(type: string | undefined, url: string | undefined): string {
  const normalized = type?.trim().toLowerCase();
  if (normalized === "streamable-http" || normalized === "streamable_http") return "http";
  if (normalized) return normalized;
  return url?.trim() ? "http" : "stdio";
}

/**
 * 将未知值收窄为普通对象。
 *
 * @param value 待判断值
 * @returns 普通对象或 undefined
 */
function asRecord(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}

/**
 * 读取字符串值。
 *
 * @param value 待判断值
 * @returns 字符串或 undefined
 */
function readString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

/**
 * 读取可空字符串值。
 *
 * @param value 待判断值
 * @returns 字符串、null 或 undefined
 */
function readNullableString(value: unknown): string | null | undefined {
  return value === null ? null : readString(value);
}

/**
 * 读取布尔值并提供默认值。
 *
 * @param value 待判断值
 * @param fallback 默认值
 * @returns 布尔值
 */
function readBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

/**
 * 读取字符串数组。
 *
 * @param value 待判断值
 * @returns 字符串数组或 undefined
 */
function readStringArray(value: unknown): string[] | undefined {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
    ? value as string[]
    : undefined;
}

/**
 * 读取字符串键值对象。
 *
 * @param value 待判断值
 * @returns 字符串键值对象或 undefined
 */
function readStringRecord(value: unknown): Record<string, string> | undefined {
  const record = asRecord(value);
  if (!record || Object.values(record).some((item) => typeof item !== "string")) return undefined;
  return record as Record<string, string>;
}

/**
 * 读取可空数值。
 *
 * @param value 待判断值
 * @returns 数值、null 或 undefined
 */
function readNullableNumber(value: unknown): number | null | undefined {
  if (value === null) return null;
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

/**
 * 生成不与现有服务冲突的默认 id。
 *
 * @param servers 已有服务列表
 * @returns 新服务 id
 */
export function uniqueServerId(servers: McpServerConfig[]): string {
  let suffix = servers.length + 1;
  let id = `server-${suffix}`;
  while (servers.some((server) => server.id === id)) {
    suffix += 1;
    id = `server-${suffix}`;
  }
  return id;
}

/**
 * 生成服务列表副文案。
 *
 * @param transport 传输方式
 * @param server 服务配置
 * @param t 双语函数
 * @returns 列表 meta 文本
 */
export function transportMeta(
  transport: string,
  server: McpServerConfig,
  t: (en: string, zh: string) => string
): string {
  if (transport === "stdio") {
    const command = [server.command, ...(server.args ?? []).slice(0, 1)].filter(Boolean).join(" ");
    return command || t("stdio", "stdio");
  }
  return server.url || transport;
}

/**
 * 创建默认 stdio MCP 服务草稿。
 *
 * @param id 服务 id
 * @returns 服务配置
 */
export function createDefaultMcpServer(id: string): McpServerConfig {
  return {
    id,
    enabled: true,
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "."],
    env: {},
    cwd: null,
    url: null,
    message_url: null,
    headers: {},
    timeout_ms: 30_000
  };
}
