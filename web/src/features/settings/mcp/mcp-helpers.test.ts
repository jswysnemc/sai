import { describe, expect, it } from "vitest";
import { parseMcpJson } from "./mcp-helpers";

describe("parseMcpJson", () => {
  it("使用标准 mcpServers 对象键补充缺失的 id", () => {
    const config = parseMcpJson(JSON.stringify({
      mcpServers: {
        filesystem: {
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-filesystem", "."]
        }
      }
    }));

    expect(config.servers).toEqual([
      expect.objectContaining({
        id: "filesystem",
        enabled: true,
        transport: "stdio",
        command: "npx"
      })
    ]);
  });

  it("保留显式 id 并归一化标准 type 与 disabled 字段", () => {
    const config = parseMcpJson(JSON.stringify({
      mcpServers: {
        alias: {
          id: "remote-main",
          type: "streamable-http",
          url: "https://example.com/mcp",
          disabled: true
        }
      }
    }));

    expect(config.servers?.[0]).toEqual(expect.objectContaining({
      id: "remote-main",
      enabled: false,
      transport: "http",
      url: "https://example.com/mcp"
    }));
  });
});
