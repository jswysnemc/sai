# 多信道通信设计方案（网关 Agent 转发模型）

## 一、 设计原则：保持极简，避免底层复杂化

用户在微信与 Sai 交互时，微信端已经运行着一个具备大模型推理能力的**网关 Agent（Gateway Agent）**。
不需要在底层的网络协议、长轮询循环或事件队列中实现复杂的状态机拦截器、短码映射和进程阻塞通信。

**核心方案**：
**以“网关 Agent 作为转发前台”**。
1. **微信 -> 非网关会话（入站转发）**：
   - 微信用户发送指令给微信网关 Agent。
   - 网关 Agent 理解用户意图（例如“转告当前正在写代码的终端会话”、“查一下本地会话进度”），通过 Sai 原生的跨会话通信工具（Mesh 工具组）直接与目标非网关会话通信，并将获取到的结果在微信回复中反馈给用户。
2. **非网关会话 -> 微信（出站推送）**：
   - 本地终端或后台任务等非网关会话，需要主动向微信用户通知进度或结果时，直接调用通用的 `send_channel_message` 工具，向最近活跃的微信目标（`load_latest_channel_context`）发送文本、日志或图片。

---

## 二、 核心落地步骤

### 1. 通用化 `send_channel_message` 工具
- **现状**：`send_channel_message` 已在 `src/gateways/channel_tools.rs` 实现完整，但仅在网关入站和定时任务中注册。
- **改动**：
  - 在 `src/tools/mod.rs`（包括 `register_interactive_tools` 和 `builtin_registry_without_mcp`）中注册该工具。
  - 在非网关场景下调用时，`ActiveChannelTarget` 默认自动回退至 `load_latest_channel_context(paths, "weixin")`，使得普通终端 Agent 也能直接发微信。

### 2. 向网关 Agent 注入跨会话通信能力（Mesh 工具组）
- **现状**：`src/gateways/weixin_bot/server.rs` 启动 Agent 时只调用了基础工具集，未注册 `mesh` 工具。
- **改动**：
  - 在 `server.rs` 的 Agent 工具注册流程中，挂载 `crate::tools::mesh::register`（配置 `cross_session: true`）。
  - 网关 Agent 获得如下跨会话工具能力：
    - `session_probe`：探测宿主机当前存在哪些运行中的会话及其最近状态；
    - `send`：向指定会话发送消息；
    - `session_activate`：向指定会话激活并投递指令。

### 3. 网关 Agent 提示词指引（Prompt Guidance）
- 在 `src/gateways/weixin_bot/prompt.rs` 的 `channel_prompt` 中补充指引：
  - “你可以作为微信与宿主机本地会话之间的通信桥梁。当用户询问本地会话状态、或要求向本地会话转交任务/指令时，你可以使用 mesh 工具（如 `session_probe` 查询会话、`send` 转发消息）与本地会话进行交互，并将结果回复给微信用户。”

---

## 三、 文件改动范围

1. `src/gateways/channel_tools.rs`：解耦强制性的入站当前目标，支持在非网关会话中自动解析最近渠道上下文。
2. `src/tools/mod.rs`：在普通交互会话中挂载 `send_channel_message`。
3. `src/gateways/weixin_bot/server.rs`：为微信网关 Agent 注册 `mesh` 跨会话工具组。
4. `src/gateways/weixin_bot/prompt.rs`：在系统提示词中增加跨会话转发引导。
5. 运行 `cargo test --bin sai` 保证全量回归通过。
