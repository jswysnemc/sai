# Sai

[English](./README.md)

Sai 是本地优先的 AI Agent，覆盖终端与浏览器：多供应商对话、工具调用、工作区会话、QQ/微信网关、长期记忆、Hooks 与 MCP 工具。

仓库：https://github.com/SHORiN-KiWATA/Sai

## 功能

- **CLI / TUI / Web**：同一 Rust 二进制；交互 REPL、单次 ask、内嵌 Web 工作台
- **Agent**：内置代码 / 探索 / 网关 Agent；按入口默认；TUI 与网关支持 `/agent` 切换
- **工具**：渐进式加载，shell、文件、搜索、记忆、知识库、cron 等
- **Hooks**：生命周期钩子（`agent_*`、`turn_*`、`message_*`、`tool_execution_*`），支持 shell 与 HTTP
- **MCP**：stdio / HTTP / SSE；工具名 `mcp_<server>_<tool>`；`mcp_manager` 查看状态与测试
- **会话**：多工作区树、分支对话、Web 端按会话隔离模型偏好
- **网关**：QQ / 微信机器人，默认使用网关向 Agent
- **记忆**：长期事实与往事，Web 可管理

架构说明：[ARCHITECTURE.md](./ARCHITECTURE.md)

## 环境

- Rust stable（edition 2021）
- Node.js 20+（仅构建 Web 前端时需要）
- Linux / macOS / Windows（网关与 shell hook 因平台而异）

## 构建

```bash
# 后端
cargo build --release

# Web UI（编译进二进制的 web/dist）
cd web && npm install && npm run build && cd ..
cargo build --release
```

发布产物一般在 `target/release/sai`。

## 快速开始

```bash
# 交互 REPL（TUI）
sai

# 单次提问
sai ask "总结这个仓库"

# Web 工作台
sai web

# 配置界面（TUI）
sai config
```

供应商与模型在 Sai 配置目录中设置（见 `sai config` 或 Web **设置**）。默认 Agent：

| 入口 | 默认 | 说明 |
|------|------|------|
| CLI | 内置 Sai 提示词 | Arch / 桌面助手人设 |
| TUI / Web | `general`（代码 Agent） | 工程向 |
| 网关 | `gateway` | IM 短回复 |

## 配置要点

### Hooks

```json
{
  "hooks": {
    "enabled": true,
    "items": [
      {
        "name": "log-end",
        "enabled": true,
        "event": "agent_end",
        "kind": "command",
        "script": "echo \"$SAI_HOOK_EVENT $SAI_SESSION_ID\" >> /tmp/sai-hooks.log"
      }
    ]
  }
}
```

环境变量：`SAI_HOOK_EVENT`、`SAI_HOOK_NAME`、`SAI_SESSION_ID`、`SAI_WORKDIR`、`SAI_TOOL_NAME`。

### MCP

```json
{
  "mcp": {
    "enabled": true,
    "servers": [
      {
        "id": "fs",
        "enabled": true,
        "transport": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
      },
      {
        "id": "remote",
        "enabled": true,
        "transport": "http",
        "url": "http://127.0.0.1:3000/mcp"
      }
    ]
  }
}
```

Web **设置 → Hooks 与 MCP** 可编辑相同字段。

## TUI 斜杠命令（节选）

| 命令 | 说明 |
|------|------|
| `/help` | 帮助 |
| `/agent` | 交互选择 Agent |
| `/agent <n>` | 按序号切换 |
| `/model` | 选择模型 |
| `/new` `/resume` | 会话管理 |
| `/compact` | 压缩上下文 |
| `/clear` | 清空会话 |

网关支持中英别名（如 `/代理`）。

## 开发

```bash
cargo check
cargo test
cd web && npm run typecheck && npm test && npm run build
```

## 许可

MIT — 见 [LICENSE](./LICENSE)。
