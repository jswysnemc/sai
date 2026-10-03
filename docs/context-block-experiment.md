# 可回读的工具结果压缩试验

试验分支：`experiment/context-block-compression`。

本实现借鉴 [billion-context-pi](https://github.com/ranxianglei/billion-context-pi/tree/1cb6340df262933c8e640a7f45e19a7b34fe1ed8) 的主模型提交摘要、稳定引用和按需回读机制，直接接入 Sai 的 Rust 请求链路。

## 启用

在 Sai 的 `config.jsonc` 中合并以下配置，保留已有 `context` 字段：

```jsonc
{
  "context": {
    "experimental_context_blocks": true
  }
}
```

重新启动或重新加载配置后生效。默认关闭；只适用于启用工具的内置模型引擎。普通、渐进加载、Jev 和 DeepSeek 锚定模式均可直接使用四个实验工具。关闭开关后停止注册和应用块摘要，归档仍保留。全局压缩已经删除的旧轮次仍沿用现有 checkpoint 行为，不因关闭实验而恢复。

## 一次压缩

1. 完成一个子任务后，模型调用 `context_status`，获取稳定 `message_id`、`eligible`、保护原因和当前 `revision`。摘要只应涵盖模型实际读过的内容，状态预览不是完整原文。
2. 模型用 `compress_context` 选择结果并提交事实摘要，不另外调用摘要模型。
3. Sai 校验引用、修订号及节省量，读取完整结果，将原文和摘要一起写入 SQLite 事务。
4. 下一次请求只替换所选工具结果正文。用户消息、助手消息、工具调用、结果配对及附件保持原有结构。
5. 需要细节时先调用 `search_context` 定位引用，再用 `restore_context` 分页回读。原文页进入当前工具结果，不展开全部历史。

模型请求的估算占用达到窗口一半时，每个用户轮次最多追加一次压缩提醒。模型自行选择已完成的工作；提醒不保证一定触发压缩。现有陈旧结果裁剪、全局摘要和溢出恢复继续工作。请求在维护前后重新应用块摘要，并同步更新占用估算。

示例参数，引用和版本应取自真实状态结果：

```json
{
  "message_ids": ["call_123", "call_124"],
  "topic": "登录配置检查",
  "summary": "已检查 src/auth/config.rs。超时值为 30 秒；配置解析通过，尚未验证远程服务连通性。",
  "expected_revision": 0
}
```

## 工具与限制

| 工具 | 参数与输出 |
| --- | --- |
| `context_status` | `offset`、`limit`、`block_offset` 可选。默认 20 个候选，最多 50 个；每页最多 5 个摘要。分别返回 `next_offset` 和 `next_block_offset`，可继续读取完整列表 |
| `compress_context` | `message_ids` 为 1–32 个不重复引用；`topic` 为 1–120 字符，`summary` 为 1–6000 字符；`expected_revision` 必填 |
| `search_context` | `query` 为 1–200 字符，`limit` 默认 10、最多 20；返回原文附近最多 400 字符的片段及引用 |
| `restore_context` | `message_id` 必填，`offset` 默认 0，`limit` 默认 4000、最多 8000；偏移按 Unicode 字符计算，按 `next_offset` 继续读取 |

- 最近四次工具调用、失败或未完成结果、上下文管理结果以及已归档结果不能再次压缩。
- 摘要正文和引用开销必须比原可见结果至少少 128 个估算 token。统计不包括查询状态、提交摘要和回读带来的新消息成本。
- 一次提交最多归档 16 MiB 原文。文件缺失、超过预算，或只有已截断预览而无完整引用时，整次提交失败，不保留部分摘要。
- 相同引用顺序、主题及摘要的重复提交返回同一个块，包括并发重试；不同提交必须使用最新版本。冲突后重新查询状态。
- 归档只对活动分支及当前权威 checkpoint 覆盖的历史开放；分组摘要的全部成员可见时才展示摘要。
- 原文存于会话 `conversation.db`，可跨重启读取。全局压缩续接归档归属；清空会话会同时删除块、原文和修订号。
- `restore_context` 精确恢复工具输出正文；附带的调用参数仅作预览，最多 2000 字符。

## 试验范围

本轮实现单层工具结果摘要，不压缩整段用户/助手对话，也不实现 T2/T3 分层合并。检索使用本地子串匹配，ASCII 字母不区分大小写；未实现插件的 BM25 与模糊评分。

摘要仍会损失信息，原文归档提供补救路径。压缩会改变相应历史位置及其后缀，可能引起供应商缓存重建。本地测试证明协议配对、事务、恢复和请求接入行为，不代表真实任务质量、缓存命中率或账单收益已经提升。

## 验证

```sh
cargo test -p sai --bin sai -- context_blocks compaction checkpoints tool_history::refresh tool_visibility context_projection
```

测试包括本地 HTTP 模型的完整工具轮次：查询候选、提交摘要、确认下一次请求替换历史结果、回读精确原文。其余用例覆盖开关、工具可见性、会话重绑定、分页、并发、缺失归档、分支隔离、重启和连续全局压缩。
