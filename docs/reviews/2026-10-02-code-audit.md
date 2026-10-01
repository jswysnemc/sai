Sai 代码审阅记录：提示词缓存、性能与正确性

审阅基线：`main`，`34e4c2dd8f92ffbcf40cdf591b1013795d58b552`，版本 `v0.2.4`。日期：2026-10-02。

发现 10 项正确性或缓存问题，以及 3 处性能热点。P1 表示会中断主要功能、误报完成或显著破坏上下文；P2 表示在特定场景下产生错误、额外成本或性能损失。

本次以完整代码库为检索范围，重点追踪原生模型请求、历史持久化、压缩、运行器、Web 事件与渲染、TUI 更新链路，并抽查配置、记忆、工具注册、插件、ACP、浏览器与网关。不是逐文件穷尽验证；抽查领域没有列出问题不代表已证明没有缺陷。未修改业务代码，未提交或发布。

**发现索引**

| 编号 | 等级 | 类型 | 问题 | 证据 |
| --- | --- | --- | --- | --- |
| F01 | P1 | 协议 | Anthropic 工具续请求丢失思考块和签名 | 实际请求捕获、官方文档 |
| F02 | P1 | 压缩 | 将字符节省量当成 token 节省量 | 源码、生产分词器数值验证 |
| F03 | P1 | 完成状态 | 不完整 Anthropic／Responses 流被判成功 | 本地模拟服务、实际 CLI |
| F04 | P1 | 超时 | Chat／Anthropic 流读取没有超时 | 本地模拟服务、实际 CLI |
| F05 | P1 | Web 同步 | SSE 订阅安装与历史读取之间丢事件 | 生产事件模块确定性复现 |
| F06 | P1 | 缓存、历史 | 工具调用前正文在跨轮重放时消失 | 实际 CLI 两轮请求比较 |
| F07 | P2 | 缓存 | Anthropic 默认请求没有开启缓存 | 两种请求模式捕获、官方文档 |
| F08 | P2 | 缓存、历史 | TODO／插件提醒未持久化 | TODO 实际请求比较；插件源码 |
| F09 | P2 | Web 恢复 | 超出事件保留范围后静默补发残缺历史 | 生产日志模块复现 |
| F10 | P2 | Web 副作用 | 正式页面残留本机调试上报 | 四处源码确认 |
| P01 | P2 | 后端性能 | 每个工具子轮重复全量分词和复制历史 | 调用链、生产分词器测量 |
| P02 | P2 | 前端性能 | 流式 Markdown 仍完整解析全文 | 完整组件服务端渲染测量 |
| P03 | P2 | 网关性能 | QQ 所有会话共享一把长时间持有的锁 | 锁范围与调用链确认 |

**F01 — Anthropic 工具续请求没有回传思考块和签名**

位置：[请求内容类型](/home/snemc/workspace/sai/src/llm/openai_compatible/request.rs:189)、[助手消息转换](/home/snemc/workspace/sai/src/llm/openai_compatible/request.rs:444)、[签名解析](/home/snemc/workspace/sai/src/llm/openai_compatible/stream_handlers.rs:402)、[响应结果组装](/home/snemc/workspace/sai/src/llm/openai_compatible/client_anthropic.rs:148)。

触发条件：开启 Anthropic thinking，模型返回 thinking、signature 和 tool_use，Sai 执行工具后发送下一次请求。

流处理器把签名保存在临时状态中，但 ChatResult 和后续请求内容类型没有承载完整 thinking／redacted_thinking 块。助手消息转换只生成 text 和 tool_use。官方协议要求同一工具轮中的助手消息按原样回传；过滤思考块会导致 400。

复现：本地服务发送 `audit-signature` 与工具调用，捕获到下一请求的助手 content 只有 `text`、`tool_use`，没有 thinking 或 signature。`client_style=default` 和 `claude` 都如此。本次没有请求真实付费 API；400 后果依据官方协议，而不是模拟服务伪造的错误。

修正方向：在结果、持久化和消息转换中保留供应商原始内容块及顺序，工具续轮逐字回传；单独保存可见 reasoning 文本不足以满足协议。

**F02 — 工具结果折叠混用了字符与 token 单位**

位置：[跳过压缩判断](/home/snemc/workspace/sai/src/agent/recovery.rs:69)、[节省字符统计](/home/snemc/workspace/sai/src/state/tool_history/maintenance.rs:144)。

`context_chars` 此时实际存储 token 数，`stats.saved_chars` 明确通过 `chars().count()` 计算。相减后可能错误地认为已经低于压缩阈值，直接重建消息并继续请求。

数值复现使用生产分词器、折叠文本函数和压缩阈值：

| 项目 | 数值 |
| --- | ---: |
| 折叠前占用 | 110,000 token |
| 模型窗口／压缩阈值 | 100,000／90,000 token |
| 被折叠文本节省字符 | 41,802 |
| 实际节省 token | 6,955 |
| 当前代码计算剩余量 | 68,198 |
| 按 token 计算剩余量 | 103,045 |

当前条件选择跳过压缩，但剩余请求仍超过模型窗口。这里验证的是实际判断公式与单位错误，没有把数值探针描述为完整模型请求压测。

修正方向：折叠后重新计算当前投影占用，或准确统计同一投影中的 token 差值；预算字段统一使用明确的 token 命名。

**F03 — 不完整流式响应会被当作成功**

位置：[Anthropic EOF 收尾](/home/snemc/workspace/sai/src/llm/openai_compatible/client_anthropic.rs:162)、[Responses 空闲与 EOF 分支](/home/snemc/workspace/sai/src/llm/openai_compatible/client.rs:498)、[Responses 最终组装](/home/snemc/workspace/sai/src/llm/openai_compatible/client.rs:568)。

触发条件：上游输出部分正文后结束 HTTP 响应，却没有发送 `message_stop`／`response.completed`。Responses 在已有部分输出后遇到空闲超时，也进入成功收尾。

复现：模拟服务分别只发送 `PARTIAL_NO_MESSAGE_STOP` 和 `PARTIAL_NO_RESPONSE_COMPLETED`，随后结束响应。当前 `sai 0.2.4` 均返回退出码 0，隔离数据库中的两个轮次也都被写成 `completed`。调用方无法区分完整答案和被截断答案；同样的收尾分支也没有证明工具参数流已经完整。

修正方向：单独记录协议终态，在缺少终态时返回中断或不完整结果，同时保留已展示文本。兼容特殊供应商的收尾规则需要明确配置，不能全局以“有正文”替代“完成”。

**F04 — Chat／Anthropic 配置超时不约束流读取**

位置：[HTTP 客户端构造](/home/snemc/workspace/sai/src/llm/openai_compatible/client.rs:76)、[Anthropic 字节读取](/home/snemc/workspace/sai/src/llm/openai_compatible/client_anthropic.rs:139)。

客户端只设置 `connect_timeout`。Chat 和 Anthropic 的 `stream.next().await` 没有读取超时；TCP 已连接、上游不再发送字节时，连接超时不起作用，传输重试也不会得到错误。

复现：配置 `timeout_seconds=1`，本地服务立即返回响应头并延迟正文。两个协议在探针 3 秒截止时仍未退出，需要探针结束进程。这个结果直接证明读取等待没有遵守所配时限；无限等待结论来自缺少读取截止机制的源码。

修正方向：为响应头等待、首个输出和后续字节空闲设置对应超时，并使取消能够终止这些等待；不要用过短的整轮硬截止误伤正常长回复。

**F05 — SSE 建立订阅时存在确定的事件遗漏窗口**

位置：[SSE 组装](/home/snemc/workspace/sai/src/web/api/runs.rs:168)、[异步安装订阅](/home/snemc/workspace/sai/src/runner/session_actor.rs:175)。

`bus.attach()` 只把 Attach 命令放入 Actor 队列，调用方马上读取日志。若此前排队的 Emit 尚未处理，历史读取看不到该事件；随后 Actor 先处理 Emit、再安装订阅，实时通道也收不到它。

确定性复现使用生产 SessionActor、EventJournal、WebEvent，并逐字提取生产 SSE 组装函数：在当前线程执行器中先排队 `run.completed`，随即创建 SSE 流，再让出执行权。结果为：

```text
queued_terminal_received=false journal_count=1
```

事件已落日志，但新连接没有收到。结束事件丢失时，页面可能持续显示运行中，直到另一次同步或刷新。

修正方向：先等待仓库已经提供的 `attach_ready()` 安装确认，再读取补发日志，并保留序号去重。

**F06 — 工具调用前的助手正文在历史重放中被置空**

位置：[本轮助手消息](/home/snemc/workspace/sai/src/agent/turn_execution.rs:10)、[持久化字段](/home/snemc/workspace/sai/src/state/tool_history/model.rs:2)、[重放助手消息](/home/snemc/workspace/sai/src/state/tool_history/projection.rs:223)。

模型返回正文与工具调用时，当前内存请求保留两者，但持久化只保存工具调用及 reasoning；重建时固定使用 `ChatMessage::assistant("", ...)`。

实际两轮 CLI 请求比较：

```text
第一轮工具返回后的请求：assistant.content = "PREAMBLE"
第二轮重放同一条消息： assistant.content = ""
```

从该消息起，历史不再与此前请求一致，后续前缀缓存复用受损，也丢失助手已经给出的说明或决策。这是请求内容差异的确认，不是供应商缓存命中率的实测。

修正方向：按模型子轮保存完整助手消息，并使即时请求和持久化重放使用同一种消息表示；加入跨用户轮的请求前缀一致性测试。

**F07 — 默认 Anthropic 请求没有开启提示词缓存**

位置：[Anthropic 请求构造](/home/snemc/workspace/sai/src/llm/openai_compatible/client_anthropic.rs:17)、[Claude 模拟请求重塑](/home/snemc/workspace/sai/src/llm/openai_compatible/claude_style.rs:165)。

实际捕获 `default` 和 `claude` 两种客户端风格的全部请求：顶层、system、tools 和 messages 均无 `cache_control`。固定 session_id 或 billing header 不等同于开启 Anthropic 缓存。

官方文档要求通过顶层 `cache_control` 开启自动缓存，或在内容块设置显式缓存断点。因此，默认接官方 API 时，保持前缀稳定仍无法获得该功能。用户通过 `extra_body` 自行补充、或代理代为注入缓存标记的情况不在此结论内。

修正方向：按实际供应商能力生成缓存设置，覆盖 system、工具和增长中的历史；保留用户自定义策略。

**F08 — TODO 与插件提醒只追加到内存，下一轮消失**

位置：[TODO 提醒追加](/home/snemc/workspace/sai/src/agent/turn_tools.rs:644)、[插件提醒追加](/home/snemc/workspace/sai/src/agent/tool_policy.rs:16)、[尾部 system 转 user](/home/snemc/workspace/sai/src/agent/message_context.rs:23)。

这些提醒会实际发给模型，却没有写入轮次消息表。下轮从数据库重建后，这条消息缺失，后续历史位置也随之变化。

TODO 复现：创建未完成事项，执行三个不同文件读取，再开始下一用户轮。六次模型请求中，提醒是否存在依次为：

```text
false, false, false, false, true, false
```

插件 `after_tool_policies` 存在相同追加方式，结论依据源码，没有宣称已运行所有插件。

修正方向：按工具序号保存模型已经接收的提醒，在重放时恢复原始位置、角色和内容。

**F09 — 日志保留上限导致残缺补发，却没有恢复缺口提示**

位置：[日志上限](/home/snemc/workspace/sai/src/web/runs/journal.rs:7)、[读取补发内容](/home/snemc/workspace/sai/src/web/runs/journal.rs:113)、[前端序号接受规则](/home/snemc/workspace/sai/web/src/features/chat/session-run-scope.ts:35)。

内存日志最多保留 2048 条或 16 MiB，落盘日志也会压缩。`events_after` 只返回现存记录，没有判断客户端序号是否早于保留边界。`stream.lagged` 只覆盖实时观察者被摘除，不覆盖这类历史缺口；前端接受更大的序号，也不检查连续性。

生产日志模块复现：

```text
published=3000 replayed=2048 earliest_sequence=953 gap_marker=false
```

刷新或长时间断线后，仍在生成的长回复可能只恢复尾部，并遗漏此前的状态事件。完整轮次结束后的时间线可恢复最终持久化内容，不能据此认为运行中的增量恢复完整。

修正方向：返回保留边界和明确缺口信息，缺口时恢复完整运行快照，再订阅其后的增量。

**F10 — 正式前端存在四处硬编码调试上报**

位置：[聊天页](/home/snemc/workspace/sai/web/src/features/chat/chat-page.tsx:589)、[事件 reducer](/home/snemc/workspace/sai/web/src/features/chat/run-event-reducer.ts:276)、[会话投影](/home/snemc/workspace/sai/web/src/features/chat/conversation-display.ts:28)、[SSH 卡片](/home/snemc/workspace/sai/web/src/features/ssh/ssh-secret-card.tsx:58)。

这些位置直接向 `http://127.0.0.1:7368/ingest/...` 调用 fetch，没有开发模式开关。载荷包含 SSH 主机标签、请求标识和运行状态；页面重渲染和历史事件重放会重复触发。其中 reducer 和渲染函数内的网络副作用还破坏了可重复执行的语义。

这里的目标是访问者本机，不是远程开发服务器；请求能否送达取决于本机服务与浏览器跨域限制。问题是正式代码无条件尝试该上报，以及高频渲染中的重复请求。

修正方向：移除遗留上报，确有需要的诊断统一放入显式调试开关和事件处理层。

**P01 — 工具循环重复计算整份历史**

位置：[压缩前全量投影](/home/snemc/workspace/sai/src/agent/recovery.rs:35)、[请求前再次投影](/home/snemc/workspace/sai/src/agent/turn_tools.rs:94)、[投影复制](/home/snemc/workspace/sai/src/state/request_projection/builder.rs:26)、[token 与字符估算](/home/snemc/workspace/sai/src/state/request_projection/estimate.rs:20)。

从第二个工具子轮起，压缩检查先构造全量投影、复制消息、分词并序列化；即便随后使用 API usage 替换估算值，这些成本已经发生。之后请求校验又构造一次完整投影。没有 usage 时，occupancy 路径还会再次全量估算。

生产 `token_counter` 使用 release 依赖、预热后重复 12 次的单次均值：

| 文本字节 | token | 单次分词 |
| ---: | ---: | ---: |
| 41,000 | 10,500 | 1.12 ms |
| 328,000 | 84,000 | 9.00 ms |
| 1,312,000 | 336,000 | 36.29 ms |

33.6 万 token 场景，仅两次重复分词就约 72.6 ms，尚未计入复制、JSON 序列化、图片处理和数据库工作。此数值是本机固定语料测量，不是线上整轮延迟。同步计算也占用当前异步执行线程。

修正方向：预算短路先于昂贵投影；复用同一轮投影；按稳定消息缓存估算，只对新增尾部计算；工具配对校验不应强制计算整份 token。

**P02 — 流式 Markdown 每次采用全文重新解析**

位置：[延后更新](/home/snemc/workspace/sai/web/src/features/chat/markdown-renderer.tsx:26)、[完整 Markdown 组件](/home/snemc/workspace/sai/web/src/features/chat/markdown-content.tsx:150)。

`useDeferredValue` 延后提交，memo 只在 source 不变时跳过；source 每次增长后，ReactMarkdown 仍处理全文，启用完整 GFM、数学与 KaTeX 插件。

通过 Vite 加载生产 MarkdownContent，以流式标志渲染含表格和公式的固定文本，每组预热后四次均值：

| 源文本字符 | 完整组件服务端渲染 |
| ---: | ---: |
| 1,370 | 19.05 ms |
| 6,850 | 64.13 ms |
| 27,400 | 259.21 ms |

这是包含 React 服务端渲染开销的诊断测量，不能直接推算浏览器帧率；它验证了复杂长文反复处理的成本。实际浏览器阻塞时间还需要客户端性能记录。

修正方向：缓存已经闭合的 Markdown 块，只解析变化尾部；避免对已完成公式和表格重复转换，同时验证流式语法边界不会改变已有排版。

**P03 — QQ 网关按整个处理器串行执行所有会话**

位置：[处理器锁](/home/snemc/workspace/sai/src/gateways/qq_bot/processor.rs:35)、[完整持锁范围](/home/snemc/workspace/sai/src/gateways/qq_bot/processor.rs:216)。

一把 `agent_lock` 覆盖认证、附件下载、完整模型和工具执行，以及最终回复发送。不同 QQ 用户或群即使映射到不同 Sai 会话，也无法并行处理；一个慢模型请求会阻塞后续所有消息，快捷命令同样需要等锁。

这是源码可以确定的并发上限，没有对真实 QQ 服务进行负载测试。单用户部署不一定能观察到排队问题。

修正方向：根据目标会话串行化，把共享渠道状态改成请求范围数据；仅缩短锁而保留全局渠道可变状态会产生串话风险。

**其他会改变缓存前缀的现有策略**

以下是已确认行为，不计入上述 10 项缺陷；是否调整取决于期望的成本策略：

| 行为 | 位置 | 对缓存的影响 |
| --- | --- | --- |
| 上下文达到 60% 后改写旧工具结果 | [阈值](/home/snemc/workspace/sai/src/state/compaction/budget.rs:17)、[结果维护](/home/snemc/workspace/sai/src/state/tool_history/maintenance.rs:64) | 从最早被改写结果起，历史前缀改变；没有摘要模型调用，不代表没有重新处理输入的费用 |
| 正式压缩后替换摘要与历史 | [基础上下文投影](/home/snemc/workspace/sai/src/state/request_projection/builder.rs:106) | 压缩本身会改变前缀，应视为新的缓存阶段 |
| DeepSeek 锚定首轮晋升 | [提示替换](/home/snemc/workspace/sai/src/agent/turn_tools.rs:662)、[工具可见性](/home/snemc/workspace/sai/src/agent/tool_visibility.rs:139) | 首次晋升会改变 system 和 tools，是显式阶段切换 |

多密钥轮询另有条件性风险：若密钥属于不同供应商缓存隔离域，轮询会分散复用；本次没有核实用户实际账户归属，因此不计为确定缺陷。

同时确认：运行日期、目录和模式通过历史末端状态更新；已加载 skills 通过资源更新注入；普通渐进工具使用固定网关；工具定义按注册顺序输出。这些已有机制不应误报为每轮随机破坏前缀。

**验证与复现材料**

- 现有 Rust 协议测试：58 项通过。
- 现有 SessionActor 测试：5 项通过。
- 定向前端测试：4 个文件、56 项通过。
- 独立协议探针使用当前源码重新构建的 debug 二进制，并校验版本为 `sai 0.2.4`；使用隔离 XDG 目录、虚构密钥和本机服务，没有使用真实供应商密钥。协议探针不用于性能计时。
- 事件探针直接引用生产事件模块和原始 SSE 函数；未使用的 RunnerEvent／EventAssembler 路径采用最小占位接口。
- 分词与预算探针使用生产分词器、预算函数和折叠文本。
- 现有测试通过不覆盖上述反例；独立探针用于确认当前缺陷，不能直接替代修复后的回归测试。

材料目录：[/tmp/sai-audit-20261002](/tmp/sai-audit-20261002)。

| 材料 | 内容 |
| --- | --- |
| [protocol_probe.py](/tmp/sai-audit-20261002/protocol_probe.py) | 协议、缓存字段、跨轮正文和 EOF 探针 |
| [protocol-results.json](/tmp/sai-audit-20261002/protocol-results.json) | 协议探针结果 |
| [extra_protocol_probe.py](/tmp/sai-audit-20261002/extra_protocol_probe.py) | TODO 提醒和读取超时探针 |
| [extra-protocol-results.json](/tmp/sai-audit-20261002/extra-protocol-results.json) | 附加协议结果 |
| [build_rust_probes.py](/tmp/sai-audit-20261002/build_rust_probes.py) | 事件与分词探针编译入口 |
| [event_probe-results.txt](/tmp/sai-audit-20261002/event_probe-results.txt) | 订阅空隙与历史裁剪结果 |
| [budget_probe.rs](/tmp/sai-audit-20261002/budget_probe.rs) | 预算单位反例源码 |
| [budget_probe-results.txt](/tmp/sai-audit-20261002/budget_probe-results.txt) | 字符／token 反例结果 |
| [token_probe-results.txt](/tmp/sai-audit-20261002/token_probe-results.txt) | 分词测量 |
| [markdown_probe.mjs](/tmp/sai-audit-20261002/markdown_probe.mjs) | 完整 Markdown 组件测量 |
| [markdown-results.json](/tmp/sai-audit-20261002/markdown-results.json) | Markdown 测量结果 |

复现入口：

```bash
uv run --no-project /tmp/sai-audit-20261002/protocol_probe.py
uv run --no-project /tmp/sai-audit-20261002/extra_protocol_probe.py
uv run --no-project /tmp/sai-audit-20261002/build_rust_probes.py
/tmp/sai-audit-20261002/budget_probe
node /tmp/sai-audit-20261002/markdown_probe.mjs
```

官方协议依据，通过 smart-search 获取并保存在材料目录：

- [Anthropic Prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)：自动与显式缓存均要求 `cache_control`。
- [Thinking in tool and multi-turn workflows](https://platform.claude.com/docs/en/build-with-claude/thinking-tool-workflows)：工具续轮必须原样保留思考块。

```bash
smart-search fetch https://platform.claude.com/docs/en/build-with-claude/prompt-caching --format json
smart-search fetch https://platform.claude.com/docs/en/build-with-claude/thinking-tool-workflows --format json
```

初次审阅报告写入 `docs/reviews/`，当时没有修改业务代码。后续修复状态如下。

**修复进度**

| 项目 | 状态 | 验证 |
| --- | --- | --- |
| F03、F04 | 已修复 | 缺失结束事件及 explicit incomplete 返回错误；读取空闲超时；无末尾换行的结束标记；持续输出超过读取时限。协议测试 63 项通过 |
| F01、F06、F07、F08 | 待处理 | 原始助手内容、缓存标记和提醒持久化 |
| F02、P01 | 待处理 | token 预算统一与投影计算复用 |
| F05、F09、F10 | 待处理 | 订阅确认、快照恢复和调试代码清理 |
| P02、P03 | 待处理 | Markdown 增量渲染与网关会话锁 |
