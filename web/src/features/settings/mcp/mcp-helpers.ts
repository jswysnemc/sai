import type { McpConfig, McpServerConfig } from "../../../api/contracts";

/**
 * 解析 MCP JSON 文本为配置对象。
 *
 * @param raw JSON 文本
 * @returns MCP 配置
 */
export function parseMcpJson(raw: string): McpConfig {
  const value = JSON.parse(raw) as McpConfig;
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("MCP configuration must be a JSON object");
  }
  if (value.servers !== undefined && !Array.isArray(value.servers)) {
    throw new Error("mcp.servers must be an array");
  }
  return value;
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
