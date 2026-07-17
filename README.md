# Sai

[中文文档](./README.zh-CN.md)

Sai is a local-first AI agent for the terminal and browser: multi-provider chat, tool use, session workspaces, message gateways (QQ / WeChat), long-term memory, Hooks, and MCP tools.

Repository: https://github.com/SHORiN-KiWATA/Sai

## Features

- **CLI / TUI / Web**: one Rust binary, interactive REPL, single-shot ask, and embedded web workbench
- **Agents**: built-in code, explore, and gateway agents; per-surface defaults; `/agent` switch in TUI and gateways
- **Tools**: progressive tool loading, shell, files, search, memory, knowledge base, cron, and more
- **Hooks**: lifecycle hooks (`agent_*`, `turn_*`, `message_*`, `tool_execution_*`) via shell or HTTP
- **MCP**: stdio / HTTP / SSE servers; tools exposed as `mcp_<server>_<tool>`; `mcp_manager` for status and tests
- **Sessions**: multi-workspace trees, branch (fork) conversation, per-session model preference in Web
- **Gateways**: QQ and WeChat bots with a gateway-tuned agent profile
- **Memory**: long-term facts / episodes with Web management UI

Architecture notes (Chinese): [ARCHITECTURE.md](./ARCHITECTURE.md)

## Requirements

- Rust stable (edition 2021)
- Node.js 20+ (only when building the Web UI)
- Linux / macOS / Windows (platform support varies for gateways and shell hooks)

## Build

```bash
# Backend
cargo build --release

# Web UI (embedded into the binary via web/dist)
cd web && npm install && npm run build && cd ..
cargo build --release
```

The release binary is typically at `target/release/sai`.

## Quick start

```bash
# Interactive REPL (TUI)
sai

# One-shot question
sai ask "summarize this repo"

# Web workbench
sai web

# Config UI (TUI)
sai config
```

Place provider credentials and models in Sai’s config directory (see `sai config` / Web **Settings**). Default agents:

| Surface | Default agent | Role |
|---------|---------------|------|
| CLI | built-in Sai prompt | Arch / desktop helper persona |
| TUI / Web | `general` (code agent) | engineering agent |
| Gateway | `gateway` | short IM-oriented replies |

## Configuration highlights

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

Hook env vars: `SAI_HOOK_EVENT`, `SAI_HOOK_NAME`, `SAI_SESSION_ID`, `SAI_WORKDIR`, `SAI_TOOL_NAME`.

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

Web **Settings → Hooks & MCP** can edit the same fields.

## TUI slash commands (selected)

| Command | Description |
|---------|-------------|
| `/help` | help |
| `/agent` | pick agent interactively |
| `/agent <n>` | switch agent by index |
| `/model` | pick model |
| `/new` `/resume` | session management |
| `/compact` | compact context |
| `/clear` | clear conversation |

Gateways also support English and Chinese slash aliases (e.g. `/代理`).

## Development

```bash
cargo check
cargo test
cd web && npm run typecheck && npm test && npm run build
```

## License

MIT — see [LICENSE](./LICENSE).
