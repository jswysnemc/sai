# Sai Codex Agent ACP Sidecar

该 Sidecar 以透明 JSONL 代理方式运行固定版本的 Codex ACP 适配器。标准 ACP
请求与通知原样转发，仅在 `initialize` 响应中声明 Sai 已真实接通的集成能力。

当前固定 `@agentclientprotocol/codex-acp` `1.1.7`。标准错误输出直接转发，标准
输出只承载 ACP JSON-RPC。

```bash
npm ci
npm run check
node src/index.js
```
