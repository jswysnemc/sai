#!/usr/bin/env node

import { runAcp } from "@agentclientprotocol/claude-agent-acp";
import { Console } from "node:console";
import { applySaiSessionExtensions } from "./session-extensions.js";

/**
 * 启动 Sai 维护的 Claude ACP Sidecar。
 *
 * 标准会话、工具、权限、MCP 和内容更新由 ACP 适配层处理；package overrides
 * 保证适配层与本入口共享同一个新版 Claude Agent SDK。
 *
 * @returns {void}
 */
function main() {
  // stdout 仅承载 ACP JSON-RPC，诊断信息统一写入 stderr
  globalThis.console = new Console({ stdout: process.stderr, stderr: process.stderr });
  const { agent } = runAcp();
  applySaiSessionExtensions(agent);
  process.stdin.resume();
}

main();
