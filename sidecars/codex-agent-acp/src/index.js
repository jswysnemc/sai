#!/usr/bin/env node

import { spawn } from "node:child_process";
import { realpathSync } from "node:fs";
import { createRequire } from "node:module";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";
import {
  extendInitializeResponse,
  trackInitializeRequest,
} from "./capability-extensions.js";

/**
 * 启动固定版本的 Codex ACP，并透明转发 JSONL 协议流。
 *
 * @returns {void}
 */
export function main() {
  // 【Codex ACP Sidecar】【启动流程】1. 解析并启动固定版本适配器
  const adapter = resolveAdapterLaunch();
  const child = spawn(adapter.program, [...adapter.args, ...process.argv.slice(2)], {
    env: process.env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  // 【Codex ACP Sidecar】【启动流程】2. 建立客户端、agent 与标准错误三条转发链路
  const initializeIds = new Set();
  const input = relayClientInput(child, initializeIds);
  relayAgentOutput(child, initializeIds);
  child.stderr.pipe(process.stderr);
  installSignalForwarding(child);

  // 【Codex ACP Sidecar】【启动流程】3. 报告转发或子进程启动错误
  child.stdin.on("error", (error) => {
    if (error?.code !== "EPIPE") {
      process.stderr.write(`【Codex ACP Sidecar】【标准输入转发】失败: ${error.message}\n`);
    }
  });
  child.on("error", (error) => {
    process.stderr.write(`【Codex ACP Sidecar】【适配器启动】失败: ${error.message}\n`);
    process.exitCode = 1;
  });
  // 【Codex ACP Sidecar】【启动流程】4. 子进程结束后关闭输入读取器并透传退出状态
  child.once("exit", (code, signal) => {
    input.close();
    process.stdin.pause();
    process.exitCode = code ?? (signal ? 1 : 0);
  });
}

/**
 * 解析 Codex ACP 启动命令。
 *
 * 本地依赖已安装时直接执行入口；发布环境没有 node_modules 时使用 npx
 * 下载固定版本，避免回退到未验证的最新版本。
 *
 * @returns {{program: string, args: string[]}} 子进程程序与参数
 */
function resolveAdapterLaunch() {
  // 【Codex ACP Sidecar】【命令解析】1. 源码环境优先复用 Sidecar 已安装的固定依赖
  try {
    const require = createRequire(import.meta.url);
    return {
      program: process.execPath,
      args: [require.resolve("@agentclientprotocol/codex-acp")],
    };
  } catch {
    // 【Codex ACP Sidecar】【命令解析】2. 发布环境通过 npx 下载同一固定版本
    return {
      program: process.platform === "win32" ? "npx.cmd" : "npx",
      args: ["-y", "@agentclientprotocol/codex-acp@1.1.7"],
    };
  }
}

/**
 * 将客户端标准输入逐行转发给 Codex ACP。
 *
 * @param {import("node:child_process").ChildProcessWithoutNullStreams} child Codex ACP 子进程
 * @param {Set<string>} initializeIds 等待响应的 initialize 标识集合
 * @returns {import("node:readline").Interface} 输入行读取器
 */
function relayClientInput(child, initializeIds) {
  // 【Codex ACP Sidecar】【请求转发】1. 按 JSONL 行记录 initialize 标识并转发请求
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
  lines.on("line", (line) => {
    child.stdin.write(`${trackInitializeRequest(line, initializeIds)}\n`);
  });
  // 【Codex ACP Sidecar】【请求转发】2. 宿主输入关闭后同步结束 agent 标准输入
  lines.on("close", () => child.stdin.end());
  return lines;
}

/**
 * 将 Codex ACP 标准输出逐行转发给宿主，并扩展 initialize 响应。
 *
 * @param {import("node:child_process").ChildProcessWithoutNullStreams} child Codex ACP 子进程
 * @param {Set<string>} initializeIds 等待响应的 initialize 标识集合
 * @returns {void}
 */
function relayAgentOutput(child, initializeIds) {
  // 【Codex ACP Sidecar】【响应转发】1. 按 JSONL 行扩展匹配的 initialize 响应
  const lines = createInterface({ input: child.stdout, crlfDelay: Infinity });
  lines.on("line", (line) => {
    process.stdout.write(`${extendInitializeResponse(line, initializeIds)}\n`);
  });
}

/**
 * 将宿主终止信号转发给 Codex ACP 子进程。
 *
 * @param {import("node:child_process").ChildProcessWithoutNullStreams} child Codex ACP 子进程
 * @returns {void}
 */
function installSignalForwarding(child) {
  // 【Codex ACP Sidecar】【信号转发】1. 宿主退出时把终止信号交给实际适配器
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.on(signal, () => {
      if (child.exitCode === null && child.signalCode === null) {
        child.kill(signal);
      }
    });
  }
}

const invokedPath = process.argv[1]
  ? pathToFileURL(realpathSync(process.argv[1])).href
  : "";
if (import.meta.url === invokedPath) {
  main();
}
