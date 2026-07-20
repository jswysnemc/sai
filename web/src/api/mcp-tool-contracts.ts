/** MCP 服务发现的完整工具元数据。 */
export type McpToolInfo = {
  server_id: string;
  name: string;
  description: string;
  input_schema: Record<string, unknown>;
};
