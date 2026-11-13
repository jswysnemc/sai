# Sai 与 Codex Agent 运行时对比

## 本次实现后的变化

本文正文保留初始源码比较，以下内容描述本次改动后的实际行为，不把源码推断当作性能测量。

- 会话记忆抽取不再每轮启动。开启记忆后，新增可见用户文本、回复、工具参数与结果累计**严格超过 40,000 tokens** 才能取得后台抽取租约；成功抽取或成功压缩后更新基线。重复发送的历史提示不计入新增间隔。
- 会话记忆已进入前台、后台及缓存前缀回放压缩请求。记忆正文有独立预算，不能挤占历史与回复预留；原始记录和最新明确指令优先。记忆仍不是无条件追加到每个主模型请求的系统提示。
- 记忆写入、抽取基线及压缩应用使用事务边界。切换会话分支清除旧记忆和租约，旧后台结果不能覆盖新分支；会话重置清理会话数据。
- 压缩按实际上下文窗口判断是否需要进行，不因 40,000-token 记忆抽取间隔而推迟必要压缩；两个触发条件不能混为一谈。
- 侧栏常驻显示生成速率、上下文占用/窗口和缓存率。提供方未报告的量显示未知；速率是当前流式生成的估算值，不是跨提供方性能基准。
- 控制型同步工具 hooks 开启时，部分只读批量/流式预执行优化保守关闭，确保最终参数审查与审批不会被并发入口绕过；有安全控制才付出这项效率成本。
- ACP 与外部多内核入口、配置和前端设置已删除；内置人格提示、编码产物及回退代码已删除。保留功能型 Agent 工具档案与用户自定义提示词。

本次没有进行同模型、同上游的付费 A/B 测试。仍不能据此给出 Sai/Codex 的真实失败率、缓存命中率或速度倍数。

## 1. 比较范围与结论边界

- 探查日期：2026-11-14
- Sai：`b7541cac7e47df0d28051e9bafbd12d09d0adfe3`，比较当前工作区，包含工具悬停和 `/code-model` 默认设置持久化的未提交改动
- Codex：本地 `/home/snemc/workspace/codex`，`627b579219d492b5445d6d433d3110eb53e507dc`，工作区干净
- 本文的 Sai 路径相对本仓库根目录；`../codex/` 路径指向上述本地 Codex 仓库
- 方法：阅读双方实现、调用路径和回归测试，并运行 Sai 的相关测试；没有运行同模型、同上游的付费性能基准，也没有运行 Codex 测试
- “运行稳定性”包含断流恢复、取消、持久化和重启恢复；重复提出的“稳定性”合并到此维度

本次不能给出真实缓存命中率、整体失败率或速度倍数。源码能确认的是执行机制和边界，测试能确认的是特定回归场景。

## 2. 主要判断

| 方向 | 源码确认的差异 | 可以作出的判断 |
| --- | --- | --- |
| 模型传输稳定性 | Sai 统一回放请求并撤销部分文本；Codex 另有 WebSocket 回退和条件性的长时间重连 | Codex 在已检查的 Responses 传输路径上拥有更多恢复分支，不能据此认定所有错误都恢复得更好 |
| 工具执行稳定性 | Sai 部分结果更新使用 SQLite 事务，Codex 会话历史和 rollout 有异步写入边界；Sai 只读预执行与常规调用记录时机不同 | 两边都不能从这些机制推出外部副作用全局“恰好执行一次” |
| 缓存命中 | 两边都有稳定前缀和工具定义优化；缓存键与传输策略不同 | 不能把 Sai 说成没有缓存优化，也不能仅凭 Codex 的缓存键或 WebSocket 增量宣布命中率更高 |
| 缓存计量 | Sai Responses 用量转换固定缓存写入量为零；Codex 解析并保留该字段 | Sai 在新模型的缓存写入成本分析上存在明确观测缺口 |
| 执行效率 | Sai 主代理支持最多 8 路连续只读并发和有限流式预执行；Codex 按工具能力统一并行/独占，并在完整调用项到达时调度 | Codex 的执行重叠范围更广；具体收益取决于工具类型、模型输出顺序与审批策略 |
| 子代理效率 | Sai 子代理使用独立串行工具循环；Codex 子代理恢复进入标准线程运行路径 | Sai 主代理的并发与流式预执行收益不会自动覆盖子代理 |

各项证据和限定条件见后续章节。

## 3. 运行稳定性

### 3.1 模型请求重试与断流恢复

Sai 的统一重试方法每次重新发送同一份 `messages` 和 `definitions`。瞬时错误发生后，它通过 `Reset` 撤销本次尝试的正文和思考片段，再按配置退避重试；回调本身失败时不会自动重试。默认最多 3 次尝试，首次等待 200 毫秒，支持 `Retry-After`。这里不是只在首个字符前重试，部分旧注释与当前实现不一致，应以实现和断流测试为准。

证据：`src/llm/request_retry.rs:21`、`src/llm/request_retry.rs:43`、`src/llm/request_retry.rs:48`、`src/config/defaults.rs:215`、`src/llm/request_retry_tests.rs:74`。

Sai 主代理同时丢弃失败尝试的预执行读取，并在最终响应到达后校验调用 ID、工具、参数和顺序。它避免把未确认读取结果混入下一次尝试，但这不是回滚已经完成的外部写操作。

证据：`src/agent/message_request.rs:123`、`src/agent/message_request.rs:165`、`src/agent/streamed_tools.rs:125`、`src/agent/streamed_tools.rs:154`。

Codex 使用类型化错误选择重试时机，尊重服务端重试期限；流重试耗尽后可以从 WebSocket 回退到 HTTPS，并为新传输重新计算流重试预算。开启 `UnboundedConnectionRetries` 且满足会话来源、提供方等条件时，连接失败会按 5 秒起步、最多 60 秒的间隔持续重连。该分支不是所有错误、所有配置都无限重试。退避等待同时接入轮次取消和抢占信号。

证据：`../codex/codex-rs/core/src/responses_retry.rs:89`、`../codex/codex-rs/core/src/responses_retry.rs:94`、`../codex/codex-rs/core/src/responses_retry.rs:121`、`../codex/codex-rs/core/src/responses_retry.rs:142`、`../codex/codex-rs/core/src/session/turn.rs:2061`。

判断：Codex 的传输恢复分支更丰富；Sai 的有限重试较容易界定等待预算。重试更多可能减少需要人工恢复的次数，也可能延长最终失败前的等待，不能直接等同于吞吐或成功率更高。

### 3.2 上下文超限不能简单判定 Codex 更优

Sai 主代理在提供方明确返回上下文超限后会尝试一次强制压缩，重建请求，并补回当前轮次已经记录的工具消息，再继续工具循环。子代理也有压缩后一次重试的独立实现。恢复失败时保留失败状态，不无限压缩重试。

证据：`src/agent/turn_orchestration.rs:275`、`src/agent/turn_orchestration.rs:296`、`src/agent/turn_orchestration.rs:303`、`src/agent/turn_orchestration.rs:314`、`src/tools/subagent_runner/context.rs:104`。

Codex 有轮次前、轮次中、轮次后的压缩检查，但当前版本采样层遇到 `ContextWindowExceeded` 会设置上下文用量为满并传播错误。外层同轮强制压缩重试分支要求存在 Guardian 的已耗尽审核预算等条件；不满足该条件时进入普通错误收尾。因此，不能把它描述成所有提供方超限错误都自动压缩并恢复当前轮次。

证据：`../codex/codex-rs/core/src/session/turn.rs:261`、`../codex/codex-rs/core/src/session/turn.rs:745`、`../codex/codex-rs/core/src/session/turn.rs:917`、`../codex/codex-rs/core/src/session/turn.rs:975`、`../codex/codex-rs/core/src/session/turn.rs:2028`。

判断：在“普通提供方超限后是否自动恢复当前轮次”这一具体场景，Sai 有明确的一次恢复路径，Codex 的行为需要结合具体会话和压缩功能配置判断。预防超限与已经超限后的恢复不是同一个指标。

### 3.3 工具结果记录有局部事务，但不保证外部副作用恰好一次

Sai 常规工具路径先记录调用开始，再运行工具，完成后保存结果。流式只读预执行是例外：读取可以先于正式调用记录启动，最终确认后才进入正常结果交付流程。主代理的 `tool_results` 更新与 `tool_calls` 状态更新在同一个 SQLite 事务提交；连续只读并发组按完成时刻记录结果，慢工具不会挡住快工具的结果提交。

证据：`src/agent/tool_history.rs:17`、`src/agent/tool_history.rs:69`、`src/agent/streamed_tools.rs:87`、`src/agent/streamed_tools.rs:137`、`src/state/tool_history/repository.rs:353`、`src/state/tool_history/repository.rs:361`、`src/state/tool_history/repository.rs:388`、`src/state/tool_history/repository.rs:403`、`src/agent/tool_batch_execution.rs:209`、`src/agent/tool_batch_execution_tests.rs:13`。

这个事务只覆盖数据库结果和调用状态。工具副作用发生在结果提交之前；裁剪结果的归档文件、替换记录和最终结果更新也不是一个统一事务。进程若在“操作已生效、结果尚未记录”之间退出，会留下未知结果，不能据调用状态判断操作一定没有生效。

证据：`src/agent/tool_execution.rs:59`、`src/agent/tool_history.rs:78`、`src/agent/tool_history.rs:83`、`src/state/tool_history/repository.rs:198`。

Sai 子代理检查点恢复保留已完成结果，并为缺少结果的调用补充说明：操作可能已经生效，应检查实际状态，不能盲目重试。这是明确的未知结果处理策略，不是把中断工具自动重新执行。

证据：`src/tools/subagent_runner/checkpoint.rs:65`、`src/tools/subagent_runner/checkpoint.rs:83`、`src/tools/subagent_runner/checkpoint_tests.rs:54`。主代理重启时会把 pending 调用结算为 interrupted 并补中断结果，见 `src/state/tool_history/repository.rs:598`。

Codex 收到完整工具调用项时先记录历史，再立即调度工具。工具结果收集流程使用 `FuturesOrdered`，在流结束后按调用顺序收集并追加会话历史；工具生命周期事件和结果就绪事件可以更早出现，不能把界面已显示完成当成 rollout 写入屏障已完成。

证据：`../codex/codex-rs/core/src/stream_events_utils.rs:383`、`../codex/codex-rs/core/src/stream_events_utils.rs:388`、`../codex/codex-rs/core/src/tools/parallel.rs:341`、`../codex/codex-rs/core/src/session/turn.rs:2852`、`../codex/codex-rs/core/src/session/turn.rs:2867`、`../codex/codex-rs/core/src/session/turn.rs:3791`。

Codex 的 rollout `record_canonical_items` 是把项目送入写入队列；`persist`、`flush` 则等待写入任务确认。写入失败会保留尚未成功写入的后缀，并尝试重新打开文件。所检查的 JSONL 写入路径调用 `file.flush()`，不能把它直接解释成已经取得断电级 `fsync` 保证。

证据：`../codex/codex-rs/rollout/src/recorder.rs:1084`、`../codex/codex-rs/rollout/src/recorder.rs:1102`、`../codex/codex-rs/rollout/src/recorder.rs:1123`、`../codex/codex-rs/rollout/src/recorder.rs:1737`、`../codex/codex-rs/rollout/src/recorder.rs:1860`、`../codex/codex-rs/rollout/src/recorder.rs:2067`。

Codex 的 `executed_tool_calls` 也不能当作工具结果幂等账本。该记录器不执行或等待工具，`retry_cache` 复用的是附加到请求的执行元数据。它有证据去重、完整性和容量控制，但不能因此推导“相同外部操作在任意重试或重启中必定只执行一次”。

证据：`../codex/codex-rs/core/src/tools/executed_tool_calls.rs:1`、`../codex/codex-rs/core/src/tools/executed_tool_calls.rs:54`、`../codex/codex-rs/core/src/tools/executed_tool_calls.rs:344`、`../codex/codex-rs/core/src/tools/executed_tool_calls/request_metadata.rs:1`、`../codex/codex-rs/core/src/tools/executed_tool_calls/request_metadata.rs:16`、`../codex/codex-rs/core/src/tools/executed_tool_calls/request_metadata.rs:210`。

判断：两边都具备结果保留与恢复机制，但没有证据证明外部副作用和本地历史形成跨系统原子事务。正确表述是“不保证全局恰好一次”，不是“中断后必然重复执行”。程序退出、进程强杀、磁盘写入错误和主机断电也应分别讨论。

### 3.4 取消与命令会话

Sai 的命令守卫在 future 丢弃、超时或显式终止时清理进程树；Unix 使用独立进程组终止，Windows 使用 Job。交互调用还有 `OperationGuard`，取消输入或输出等待时移除注册记录并终止整个会话。

证据：`src/tools/command/process_lifecycle.rs:31`、`src/tools/command/process_lifecycle.rs:48`、`src/tools/command/interactive.rs:203`、`src/tools/command/interactive/session.rs:124`、`src/tools/command/interactive/session.rs:167`。

Codex 的工具运行时区分“可以直接 abort”和“取消后必须完成终态交付”的 handler，准入锁等待本身可取消。命令层既有本地 PTY 句柄，也有远程 exec-server 句柄，提供异步确认终止和 interrupt；远程输出在退出后还可能继续到达，需要额外输出关闭判断。

证据：`../codex/codex-rs/core/src/tools/parallel.rs:187`、`../codex/codex-rs/core/src/tools/parallel.rs:307`、`../codex/codex-rs/core/src/tools/parallel.rs:374`、`../codex/codex-rs/core/src/unified_exec/process.rs:93`、`../codex/codex-rs/core/src/unified_exec/process.rs:244`、`../codex/codex-rs/core/src/unified_exec/process.rs:257`、`../codex/codex-rs/core/src/unified_exec/process_manager.rs:1173`。

判断：不能把两边都支持“取消”理解为取消语义完全一致。Codex 处理的环境和 handler 生命周期更广，Sai 已有本地命令清理边界；本次没有做 Windows、远程环境或进程逃离进程组的实测。

## 4. 缓存命中机制与计量

### 4.1 两边都在维护稳定前缀

Sai 保留领先 system 消息，后续 system 内容转为 user；上下文 epoch 保存固定基线，状态或指令改变时把更新追加到尾部，运行环境和模式也只在变化时追加。渐进式工具加载路径保持网关声明，完整已加载定义进入注入消息或加载结果，而不是每次膨胀原生 tools 目录。工具定义遵循注册顺序，混合脚本模式不重复内联同一业务说明。

证据：`src/agent/message_context.rs:10`、`src/agent/system_prompt.rs:40`、`src/agent/context_projection.rs:108`、`src/agent/runtime_context.rs:77`、`src/agent/tool_visibility.rs:172`、`src/tools/registry_tests.rs:155`、`src/agent/codemode_hybrid_tests.rs:117`。

Codex 的提示缓存回归测试明确检查：权限或配置覆盖后缓存键不变，原始输入仍完整保留为新请求的前缀。该测试本次只阅读，没有运行。

证据：`../codex/codex-rs/core/tests/suite/prompt_caching.rs:551`。

判断：Sai 已经有缓存友好的前缀和定义组织策略，不能概括为“每轮全部变化，因此缓存不能命中”。模型切换、定义变化、压缩和重新加载仍可能改变有效前缀，两边都需要结合实际请求分析。

### 4.2 缓存键、连接复用与模型缓存是不同层次

Sai 根据模型、领先 system 消息和工具定义计算 SHA256 派生 UUID；在 Codex-shape Responses 请求中设置缓存键和客户端元数据。已检查的 HTTP 请求路径发送完整 `lower_messages`；它复用 `reqwest::Client`，不能说每次都重新建立 TCP 连接。

证据：`src/llm/openai_compatible/request.rs:14`、`src/llm/openai_compatible/client.rs:27`、`src/llm/openai_compatible/client.rs:75`、`src/llm/openai_compatible/client.rs:403`、`src/llm/openai_compatible/client.rs:411`、`src/llm/openai_compatible/client.rs:437`。

Codex 的默认缓存键从 session 或 parent 派生，也支持自定义和省略。满足连接状态、请求属性和输入前缀条件时，WebSocket 可以带 `previous_response_id` 并只发送增量输入；条件变化时退回完整请求。连接预热会发送 `generate=false` 的准备请求并等待完成，且预热失败仍消耗同一轮次的连接尝试预算。

证据：`../codex/codex-rs/core/src/client.rs:429`、`../codex/codex-rs/core/src/client.rs:712`、`../codex/codex-rs/core/src/client.rs:2373`、`../codex/codex-rs/core/src/client.rs:2536`、`../codex/codex-rs/core/src/client.rs:17`、`../codex/codex-rs/core/src/client.rs:25`、`../codex/codex-rs/core/src/thread_manager.rs:331`。

判断：Codex 在兼容的 Responses 服务上多了一层连接和服务端续接复用，可减少某些请求的传输量；这不等于模型 KV 缓存命中率。不能把连接池命中、WebSocket 续接、工具定义复用和模型提示缓存读命中混成一个百分比。

### 4.3 Sai 支持 Anthropic 缓存和手动扩展参数

Sai 的 Anthropic 请求默认调用自动缓存设置，顶层采用 ephemeral 策略，并尊重显式缓存设置。它还把 Anthropic 的普通输入、缓存写入和缓存读取加总为 prompt tokens。Sai 支持通过 `extra_body` 追加参数，因此不能说它完全不能配置缓存保留时长；Codex 另有类型化 TTL 设置。

证据：`src/llm/openai_compatible/client_anthropic.rs:29`、`src/llm/openai_compatible/anthropic_content.rs:80`、`src/llm/openai_compatible/stream_handlers.rs:498`、`src/llm/thinking.rs:21`、`src/llm/thinking.rs:383`、`../codex/codex-rs/core/src/client.rs:1307`。

### 4.4 最明确的缓存观测差异：Responses 缓存写入量

Sai 的 `PromptTokensDetails` 只读取 `cached_tokens`，Responses 用量转换固定 `cache_write_tokens: 0`。若上游返回 `input_tokens_details.cache_write_tokens`，此路径不会采集它。Codex 的 Responses SSE 解析会读取缓存写入，并在协议统计中保留和累计。

证据：`src/llm/openai_compatible/stream_types.rs:31`、`src/llm/openai_compatible/stream_types.rs:185`、`../codex/codex-rs/codex-api/src/sse/responses.rs:131`、`../codex/codex-rs/protocol/src/protocol.rs:2544`、`../codex/codex-rs/protocol/src/protocol.rs:2770`。

判断：这是字段覆盖和成本分析差距，尤其影响有独立缓存写入计价的模型；它不直接证明 Sai 的缓存读取命中率低于 Codex。

Sai 底栏缓存命中率采用本轮累计 `cache_read_tokens / prompt_tokens`，是输入 token 加权比例，不是请求成功率，也不自动代表整项任务包含子代理、Jev 和压缩请求的缓存率。

证据：`src/cli/repl_runtime/live_usage.rs:24`。

### 4.5 提供方官方语义限制

2026-11-14 获取的 OpenAI 官方说明强调缓存键不能保证命中；GPT-5.6 及以后不需要靠该键优化缓存路由，较早模型才推荐稳定键。工具定义、顺序和参数 Schema 应保持稳定。新模型可能对缓存写入独立计价，缓存读写应分别记录。

Anthropic 官方说明按 tools、system、messages 的层级建立缓存前缀，自动缓存会移动最后一个可缓存块；命中要求片段完全一致，默认 5 分钟，也支持 1 小时。不能直接拿 Claude、OpenAI 和不同代理网关的缓存比例作运行时排名。

官方来源：

- [OpenAI Prompt Caching](https://developers.openai.com/api/docs/guides/prompt-caching)，读取日期 2026-11-14
- [Anthropic Prompt Caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)，读取日期 2026-11-14

## 5. 运行效率的决定性差异

### 5.1 并发范围与启动时机

Sai 主代理把连续、无需审批的只读调用组成批次，最多 8 路并发，交互和写调用保留串行边界。流式提前执行额外限定为 `read_file`、`list_files`、`search_files`、`grep` 的连续前缀，最多 8 项；其他调用等待完整模型响应进入正常执行流程。

证据：`src/agent/tool_invocation.rs:88`、`src/agent/tool_invocation.rs:118`、`src/agent/tool_batch_execution.rs:12`、`src/agent/tool_batch_execution.rs:178`、`src/agent/streamed_tools.rs:63`、`src/agent/turn_tools.rs:225`。

Codex 在 `OutputItemDone` 确认完整调用项后即可调度，不要求整个响应完成，也不局限于上述只读工具前缀。它按工具是否支持并行，使用读写锁协调并行与串行执行；后置模型 hook、邮箱抢占等情况会延迟或取消调度。

证据：`../codex/codex-rs/core/src/stream_events_utils.rs:388`、`../codex/codex-rs/core/src/stream_events_utils.rs:390`、`../codex/codex-rs/core/src/tools/parallel.rs:183`、`../codex/codex-rs/core/src/tools/parallel.rs:292`。

判断：Codex 可重叠的模型输出和工具工作范围更广，Sai 的有限预执行更保守；一次只调用一个短工具时差异可能很小，长工具、多个工具和审批等待才可能放大差异。不能仅比较“并发数量”就推导总任务速度。

### 5.2 Sai 子代理没有自动继承主代理优化

Sai 子代理有自己的 `SubagentRunner`：完成模型响应后逐条执行工具，逐边界保存完整检查点，压缩也有单独实现。当前循环没有调用主代理的只读并发组或流式预执行缓存。压缩前还会估算完整消息及工具定义，不使用主代理同一套后台预计算流程。

证据：`src/tools/subagent_runner/turn.rs:70`、`src/tools/subagent_runner/turn.rs:84`、`src/tools/subagent_runner/turn.rs:143`、`src/tools/subagent_runner/turn.rs:174`、`src/tools/subagent_runner/context.rs:27`、`src/tools/subagent_runner/context.rs:55`。

Codex 恢复子代理时读回模型上下文，构造 `InitialHistory::Resumed`，进入标准线程恢复入口，同时恢复 agent 元数据和线程监听，而不是引入另一套简化模型工具循环。

证据：`../codex/codex-rs/core/src/agent/control/spawn.rs:1035`、`../codex/codex-rs/core/src/agent/control/spawn.rs:1084`、`../codex/codex-rs/core/src/agent/control/spawn.rs:1102`、`../codex/codex-rs/core/src/agent/control/resume.rs:14`。

判断：多子代理任务是 Sai 与 Codex 更容易出现效率差异的场景。Sai 不是完全缺少子代理恢复，而是恢复和执行优化走独立路径，需要分别评估，不能用主代理的测试结果代表它。

### 5.3 两边压缩均以 token 为核心，差异不在“字符数对真实 token”

Sai 主代理优先使用提供方最近的 prompt/completion 用量，并估算未计入的尾部消息；没有用量时按文本 BPE 与图片公式估算，未变化消息计数有本地指纹缓存。`context_char_budget`、`trigger_chars` 等名称保留旧术语，不代表当前触发逻辑仅统计字符。子代理会另计工具定义占用。

证据：`src/agent/recovery.rs:36`、`src/state/compaction/estimate.rs:30`、`src/state/compaction/estimate.rs:55`、`src/state/compaction/token_cache.rs:12`、`src/tools/subagent_runner/context.rs:27`。

Codex 区分整个活动上下文与初始前缀之后的正文占用，还单独判断模型硬窗口和压缩窗口；可以配置按 total 或 body-after-prefix 触发。其后台压缩受功能开关、会话来源和远程 compaction V2 能力约束，结果校验历史版本、模型、压缩哈希和认证归属。

证据：`../codex/codex-rs/core/src/session/context_window.rs:58`、`../codex/codex-rs/core/src/session/context_window.rs:68`、`../codex/codex-rs/core/src/session/context_window.rs:83`、`../codex/codex-rs/core/src/session/async_compaction.rs:42`、`../codex/codex-rs/core/src/session/async_compaction.rs:185`。

Sai 后台摘要在配置触发阈值的 85% 开始，达到阈值时可以等待已经发出的摘要；提交前重新验证来源指纹，失败后保留正常按需压缩。它还先在本地裁剪陈旧工具结果，再决定是否需要调用模型生成摘要。

证据：`src/agent/background_compaction.rs:111`、`src/agent/background_compaction.rs:127`、`src/agent/background_compaction.rs:170`、`src/agent/recovery.rs:57`、`src/agent/recovery.rs:68`。

判断：差异主要是计数范围、提供方能力、前台/后台时机和陈旧结果处理。两边都有真实用量加估算，估算误差与失效摘要请求都可能影响效率，不能把变量名当作实现结论。

### 5.4 长输出保留方式影响信息效率，不代表单纯更快

Sai 交互会话全局最多 64 个，同一归属最多 16 个；未消费输出只保留 128 KiB 尾部，超出时报告省略字节。`write_stdin` 消费该缓冲，侧栏查看不消费。因此持续大输出可能丢失开头内容。

证据：`src/tools/command/interactive.rs:18`、`src/tools/command/interactive.rs:94`、`src/tools/command/interactive/output.rs:5`、`src/tools/command/interactive/output.rs:25`、`src/tools/command/interactive/session.rs:151`、`src/tools/command/interactive/session.rs:159`。

Codex unified exec 管理器容量上限同样为 64，但输出用 1 MiB 的首尾缓冲，并分别保存待轮询输出与完成记录。它还允许按输出 token 预算裁剪交付结果。这不是同一容量口径：Sai 的限制包括全局和归属数量，Codex 的 64 是该管理器内的进程容量。

证据：`../codex/codex-rs/core/src/unified_exec/mod.rs:74`、`../codex/codex-rs/core/src/unified_exec/mod.rs:77`、`../codex/codex-rs/core/src/unified_exec/process.rs:43`、`../codex/codex-rs/core/src/unified_exec/process_manager.rs:1121`。

判断：更大缓冲和首尾保留可能减少重新获取开头信息的需要，同时增加保留内存；只保留尾部更省内存，但是否节省总任务 token 和时间取决于任务所需信息。

## 6. 本次验证与尚未测量的指标

基准调研阶段运行的 Sai 定点测试共 64 项通过，无失败；不包含此前 244 项界面测试。实现收尾阶段另外完成 Rust 工作区回归，4,716 项通过、15 项忽略，以及 Web 1,350 项测试和生产构建；真实伪终端完整回归 190 项、代码模式集成 4 项及受限网络回归通过。这些回归仍不代表受控上游性能测量。

| 测试 filter | 通过项数 | 验证内容 |
| --- | ---: | --- |
| `request_retry` | 3 | 断流回退、HTTP 状态、Retry-After、永久错误和回调错误 |
| `tool_batch_execution` | 2 | 快结果先提交、目标暂停后不启动等待中的工具 |
| `streamed_tools` | 2 | 读取提前启动且仅消费一次、丢弃响应取消未完成读取 |
| `background_compaction` | 4 | 来源指纹、当前指导保留、任务丢弃与等待取消 |
| `checkpoint` | 34 | 所有匹配该 filter 的检查点测试，含子代理恢复和写入失败 |
| `request_cache_key` | 3 | 稳定键生成和相关构造边界 |
| `message_context` | 5 | system 前缀和后续消息组织 |
| `enables_caching_without_overwriting_explicit_policy` | 1 | Anthropic 默认缓存尊重显式设置 |
| `cache_tokens` | 1 | 缓存 token 解析 |
| `live_usage` | 6 | 本轮用量累计和界面缓存比例 |
| `definitions_preserve_registration_order` | 1 | 工具声明顺序 |
| `jev_exposure_does_not_duplicate_script_catalog_descriptions` | 1 | Jev 与脚本定义去重 |
| `hybrid_session_shares_native_definitions_without_inline_duplicates` | 1 | 混合工具定义共享 |

执行方式为 `cargo test --locked -j 2 -p sai <filter>`。这些测试验证请求构造、状态归一化和故障场景，不测量真实上游提示缓存命中率。

Codex 本次只阅读源码和相关测试，没有编译或运行其测试。旧 `docs/runtime-parity-work.md` 的工作区测试总数属于历史验证记录，不作为本次对比结果。

仍未实测的核心指标：

- 相同模型、相同提供方、相同提示与工具轨迹下的 `cache_read / input`，并单列缓存写入和保留时长
- 整项任务包含主代理、子代理、压缩和工具选择模型的总调用数、输入/输出 token 与费用
- 首个工具启动时间、模型输出与工具运行的重叠时间、整项任务完成时间和尾部延迟
- 网络断流、结果提交前进程退出、磁盘写入失败等故障下的恢复成功率、未知结果工具数量和实际副作用次数
- 相同输出量与相同并发条件下的峰值内存、CPU 使用和重复读取次数

没有这些受控测量，能确认的是：Codex 的 Responses 传输与统一工具调度更丰富；Sai 已具备主要缓存和恢复机制，但子代理优化覆盖及 Responses 缓存写入计量仍有明确差距。不能把这一定性结论改写成整体稳定性排名、缓存命中率排名或速度倍数。
