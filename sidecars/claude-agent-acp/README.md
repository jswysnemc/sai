# Sai Claude Agent ACP Sidecar

该 Sidecar 为 Sai 的 Claude 外部内核提供 ACP 入口。标准能力使用 ACP，Claude
Agent SDK 无法标准化的能力只能放入 `_sai` 扩展字段。

依赖固定到 Claude Agent SDK `0.3.220`，ACP 适配器固定到
`@agentclientprotocol/claude-agent-acp` `0.63.0`；两者使用相同 SDK 版本。

```bash
npm install --package-lock=false
npm run check
node src/index.js
```
