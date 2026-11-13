# Codex hooks 能力与 Sai 补全清单

## 本次补全结果

下文“初始状态”保留调研基准，当前实现已作如下补全。Codex 与 Sai 配置结构不同，不宣称配置文件逐字兼容。

| 原差距 | 当前实现与边界 |
| --- | --- |
| 事件与匹配 | 14 个 PascalCase 事件、8 个旧观察事件及别名；规范工具名候选和正则在配置校验阶段检查 |
| 输入、决定、审批 | JSON stdin；事件专用 stdout 决定；退出 2 生效；参数改写后重新校验并按最终参数审批；后置反馈不能撤销副作用 |
| 动作与模板 | command、已注册 MCP 工具和原有 HTTP 扩展；递归模板保留 JSON 类型；未知动作拒绝加载；不实现 Codex 自身也跳过的 prompt/agent 动作 |
| 同步并发 | 同一事件的同步钩子并发执行，实际完成顺序决定最后改写；拒绝与停止优先 |
| 异步运行 | 每会话最多 8 项运行、64 项在途；只在后续安全请求点送达反馈；会话结束取消在途任务；不允许异步动作参与审批或改写 |
| 时限与清理 | 完整 stdin、stdout、stderr 与等待时限；默认 600 秒；结束/中断默认 1 秒、上限 3 秒；Unix 进程组与 Windows 进程树清理 |
| Shell 与环境 | 可指定 Shell 和 Windows 命令；会话配置快照叠加事件变量；过滤内部执行环境前缀；不记录认证材料；插件来源提供根目录与数据目录变量 |
| 来源发现与合并 | 用户 `hooks.json`、项目 `.sai/hooks.json`、已启用安装插件的 `hooks.json` 与主配置条目并存；外部条目不回写主配置；支持 Sai 列表及 Codex 风格事件分组命令格式 |
| 审查与变更校验 | 用户独立 `hooks-trust.json` 保存内容摘要；CLI/Web 展示定义后显式授权；定义解析与摘要来自同一字节快照；声明 `files` 与插件内容变化要求重新审查；每次执行前复核 |
| 安全来源读取 | 先检查普通文件，再有界读取；Unix 拒绝链接、非阻塞打开，FIFO 与超限文件不能阻塞 Agent；坏可选来源仅显示诊断 |
| 托管策略 | Unix `/etc/sai/hooks.json` 要求管理员所有权与不可由普通用户改写的祖先目录；提供 managed_only、required；托管条目不可由普通开关禁用；Windows 未引入未经验证的管理员策略来源，保持拒绝而非假定受托管 |
| 插件来源 | 只发现已安装且启用的 Sai 插件；独立审查整个插件内容；执行前重新读取摘要；不引入外部 Agent 内核 |
| 运行记录与界面 | Started/Completed/Failed、来源、会话、时间、耗时、状态说明和反馈；CLI 查询与 Web 紧凑折叠列表；日志有界轮换 |
| 输出预算 | 默认每段反馈 2,500 估算 tokens；超限正文保存到私有文件并给首尾预览与回读路径；0 不保存正文；原始输入输出仍有 1 MiB 硬上限 |
| 会话与请求边界 | 开始/恢复/清空/压缩来源、真实用户输入、重试/失败/取消、前后台压缩、主与子代理完成；Web 删除与五分钟无连接且无活动收尾；切换会话不视为结束 |
| 通知验证 | `scripts/hooks/mail-notify.py` 使用用户本地邮件脚本；标准错误隔离诊断；本地配置保留旧条目并追加异步 Stop 通知 |

验证结果：Rust 工作区 4,716 项通过、15 项忽略；Web 1,350 项通过并完成生产构建；真实伪终端完整回归 190 项通过；代码模式集成 4 项及受限网络回归通过；邮箱脚本替身测试通过。隔离模型接口通过真实主 Agent Stop 路径发送一次通知，邮件脚本成功退出。未检查收件箱，因此只确认 SMTP 发送成功，不确认最终投递。验证脚本的同步模式仅用于确保测试进程退出前完成发送；用户常驻配置使用异步模式。

保留的边界：Windows 托管策略接入尚无可信管理员来源；没有复制 Codex 插件打包体系或会话 flags 配置层级；只实现 Sai 对应的安装插件与操作系统策略来源。模型请求 hooks 不向会话标题、记忆抽取等内部辅助任务重复发送用户生命周期事件。Stop 继续执行最多介入一次，避免无条件钩子形成无限循环。

## 基准与证据

- 调研日期：2026-11-14。
- Codex 本地源码：`/home/snemc/workspace/codex`，提交 `627b579219d492b5445d6d433d3110eb53e507dc`。
- Sai 初始源码：`/home/snemc/workspace/sai`，提交 `b7541cac7e47df0d28051e9bafbd12d09d0adfe3`。本报告的差距描述针对开始调研时的实现，不代表主任务补全后的状态。
- 官方发布行为参考：[Hooks](https://developers.openai.com/codex/hooks)、[Configuration Reference](https://developers.openai.com/codex/config-reference)。两页已通过 `smart-search fetch` 获取正文；正文抽取存在代码块重复，协议细节以本地 Rust 类型和测试交叉核实。
- 当前 Codex 的 `features.hooks` 是稳定功能，默认开启；非托管 hook 仍需用户信任，不等于默认允许执行任何脚本。[C01][C02]
- 基准调研阶段只阅读源码与文档，未运行 Codex 测试、未配置真实 hook、未发送邮件；Sai 实现后的验证结果见“本次补全结果”。

## 主要差距

Sai 目前是生命周期观察器，而不是完整的控制型 hooks 引擎：

1. 只有八个 LiveAgent 风格事件。`AgentStart` 在每次 `chat_with_tools` 进入时触发，`TurnStart` 在每个工具循环触发；它们不能分别视为 Codex 的 `SessionStart`、`UserPromptSubmit`。[S01][S03]
2. 命令的 stdin 为 null；只能通过少量 `SAI_*` 环境变量获得事件、会话和工具名称。stdout 收集后丢弃，脚本不能拒绝工具、改写参数、追加上下文或要求继续执行。[S01]
3. 没有匹配器、异步配置、MCP handler、来源信任、配置分层合并、运行结果事件和大输出保存。[S01][S02]
4. 工具、代码模式、并发工具有各自的事件调用点，新增控制语义必须同时覆盖这些入口，否则相同工具通过不同入口会绕过 hooks。[S03][S04][S05]
5. 现有 Web 设置只提供八个事件、command/http 和 100–120,000 ms 超时。界面明确说明所有失败都不阻断，补全后必须同步更改字段、事件清单和说明。[S06][S07][S08]

## 逐事件对照

Codex 事件清单、匹配器适用范围来自 [C03]。下表的“阻断”只描述同步 handler；异步 handler 不控制已触发操作。

| 事件 | 触发点与匹配器 | Codex 实际作用 | Sai 需要补全 |
| --- | --- | --- | --- |
| `SessionStart` | 主会话 startup/resume/clear/compact/fork；匹配 source | 文本 stdout 或 `additionalContext` 注入；`continue:false` 阻止后续模型请求 | 真正的会话边界；压缩后紧接下一请求注入，而非每次工具循环启动 |
| `SessionEnd` | 主会话结束；匹配 reason，目前固定 `other` | 只运行收尾命令，输出不改变控制；始终同步 | 正常退出和服务会话结束入口；不等价每轮 `AgentEnd` |
| `UserPromptSubmit` | 用户输入准备发送；不使用 matcher | 文本/JSON 补充上下文；block+reason、exit 2 或 `continue:false` 拒绝本次输入 | 请求准备入口，拒绝时不能继续请求模型 |
| `PreModelRequest` | 每次模型请求尝试，包括重试；不使用 matcher | 纯观察，不能阻断或注入上下文 | 请求尝试层事件，不仅工具循环开始事件 |
| `PostModelRequest` | 每次尝试结束；不使用 matcher | 纯观察；status 为 completed/failed/cancelled，error、token_usage 可为空 | 完成、失败、取消和重试的完整对称事件 |
| `PreToolUse` | 工具开始执行前；匹配规范工具名和别名 | deny+非空理由、旧 block+reason、exit 2；补充上下文；allow+updatedInput 改写 | 串行/并发/代码模式嵌套调用的统一前置门禁 |
| `PermissionRequest` | 即将显示审批请求；匹配工具名和别名 | allow/deny；任一 deny 优先；无决定则保留正常审批 | 审批入口 hook，不可用 PreToolUse 的 allow 直接绕过审批 |
| `PostToolUse` | 已运行工具返回，包括非零 shell 退出；匹配工具名和别名 | 补充上下文；block/exit 2/continue:false 可替换模型可见结果为反馈，但不能撤销副作用 | 在结果保存和后续模型投影之间落实反馈语义，覆盖嵌套工具 |
| `PreCompact` | 压缩前；匹配 manual/auto | `continue:false` 在压缩前停止；不支持摘要输入上下文追加 | 手动、自动、溢出恢复的压缩入口 |
| `PostCompact` | 压缩成功后；匹配 manual/auto | `continue:false` 阻止压缩后的继续运行 | 只在实际应用成功后触发；后台预计算不等于压缩完成 |
| `SubagentStart` | 用户可见线程子代理启动；匹配 agent_type | 文本/JSON 为子代理补充上下文；`continue:false` 不阻止启动 | 子代理生命周期和类型、身份字段；不要向内部辅助任务重复发用户 hooks |
| `SubagentStop` | 子代理准备完成；匹配 agent_type | block+reason/exit 2 要求继续；`continue:false` 优先终止 | 子代理完成判定和继续执行通道，保留 stop_hook_active |
| `Stop` | 主轮次准备完成；不使用 matcher | block+reason/exit 2 要求继续工作；`continue:false` 优先终止 | 真正完成前的继续执行控制，不是最终完成后只发通知 |
| `Interrupt` | 用户中断主会话活动轮次；不使用 matcher | 只观察与清理；可输出 systemMessage，不得取消中断或重启任务 | 只对活动主轮次触发；不对空闲或内部子代理触发 |

事件语义源码：[C04]–[C13]。官方说明 SessionEnd 在正常关闭、开放会话归档/删除或无客户端连接后的空闲过期时运行；仅切换会话不等于会话结束。Sai 应对照自己的会话生命周期落实，不需要引入被移除的外部内核。[D01]

## 命令输入与结果协议

### 输入

命令 stdin 是一个完整 JSON 对象，公共字段采用 snake_case；`hook_event_name` 值采用事件名，例如 `PreToolUse`。不能把 `SAI_HOOK_EVENT` 环境变量视为兼容协议。[C04][C14]

- 公共上下文：`session_id`、`cwd`、`transcript_path`，大部分事件还携带 `model`、`turn_id`。
- 工具事件：`tool_name`、`tool_input`、`tool_use_id`；后置事件还有 `tool_response`。
- 用户输入：`prompt`；会话开始：`source`；压缩：`trigger`。
- Stop：`stop_hook_active`、`last_assistant_message`。
- 子代理事件：`agent_id`、`agent_type`；SubagentStop 还有 `agent_transcript_path`，并区分主会话与子代理记录路径。
- 模型请求后置事件：`status`、`error`、`token_usage`。用于统计时要区分一次尝试与整个用户轮次。[C10]

例：

```json
{
  "session_id": "session_example",
  "turn_id": "turn_example",
  "cwd": "/workspace/project",
  "transcript_path": null,
  "model": "configured-model",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_use_id": "call_example",
  "tool_input": { "command": "cargo test --locked" }
}
```

### 输出与控制

- 退出 0 且无输出视为成功；结构化 stdout 必须是一个 JSON 对象，不接受把调试日志与 JSON 混合输出。日志应写 stderr。[C15]
- `systemMessage` 是可见提示；`hookSpecificOutput.additionalContext` 是给后续模型的补充材料，两者不能混用。[C14][C15]
- 字段采用 camelCase，`hookSpecificOutput.hookEventName` 必须对应实际事件。应使用事件专用类型校验，不能让所有事件接受同一套控制效果。[C14]
- PreToolUse 的 `permissionDecision:allow` 只有同时携带 `updatedInput` 才有效；单独 allow、ask、旧 approve、continue:false、stopReason、suppressOutput 在此版本均不支持。[C15]
- 对 shell/apply_patch 的改写应包含字符串 `command`；MCP 和普通函数工具使用替换参数对象。改写后仍要走原来的审批、安全和工具参数校验。[C04][D01]
- PermissionRequest 的 allow/deny 是审批决定。updatedInput、updatedPermissions、interrupt 都不支持，不能让这些字段授予权限。官方称这类不支持字段 fail closed；本地代码记录 Failed、不给决定，随后保留普通审批，而不是无条件自动拒绝整个任务。[C05][C15]
- PostToolUse 的 block 或 exit 2 要求用 hook 反馈替换原结果并继续模型；副作用已经发生，不是“阻止工具执行”。代码模式嵌套调用里 block 可令工具 promise 拒绝；continue:false 不拒绝 promise。[C06][D01]
- Stop/SubagentStop 的 block 是“阻止结束”，其非空 reason 作为继续执行输入；continue:false 优先于其它 handler 的 block。必须保留 `stop_hook_active` 防止脚本无条件要求继续而形成无穷循环。[C07][C11]
- PreCompact/PostCompact 支持停止，不提供 additionalContext 效果；SubagentStart 只注入上下文、不控制启动；Pre/PostModelRequest、SessionEnd、Interrupt 不控制主流程。[C08][C09][C10][C12][C13]
- `suppressOutput` 在官方文档中仍是解析兼容字段，尚未实现；不应将它列作已经对齐的 Codex 能力。[D01]

## 引擎能力与补全项目

| 项目 | Codex 证据与实际边界 | Sai 初始状态 | 建议验收条件 |
| --- | --- | --- | --- |
| 事件命名与兼容 | 14 个 PascalCase 事件，[C03] | 8 个 snake_case 名称，[S01] | 提供新名称与旧配置迁移/别名，不静默丢弃已有用户 hooks |
| 匹配器 | 空、`*` 匹配全部；简单名称/竖线是精确候选；其它是编译后的正则，[C16] | 仅 event 精确相等 | 配置加载时校验正则；无效正则不可导致全部工具匹配 |
| 规范工具名与别名 | apply_patch 可匹配 Edit/Write；一个 handler 即使匹配多个别名也只执行一次，[C04][C17] | 仅当前调用 function.name | 内部路由名、规范执行名、匹配别名分开；每次调用只运行一次 |
| handler 类型 | command 与 mcp_tool 已实现；prompt/agent 解析后跳过；SessionEnd 不支持 mcp_tool，[C18] | command/http，未知 kind 回退命令 | 增加 MCP handler；保留 HTTP 作为 Sai 扩展；未知类型报配置错误，不能当命令执行 |
| MCP 输入模板 | `${field.nested}` 递归替换；完整占位符保留 JSON 类型；字段缺失失败，[C19] | 无 | 数组、对象、数值、布尔值和缺字段均有测试 |
| 同步并发 | 同一事件的匹配 handler 并发启动，按配置顺序报告；PreToolUse 多个改写取实际完成最晚者，[C17][C04] | dispatch 串行循环 | 慢观察脚本不妨碍其它匹配 handler 启动；决定合并规则确定 |
| 异步运行 | command async=true；单会话最多8个运行，其余排队；结束时取消未完成任务，[C20] | 没有用户异步字段；部分并发工具后置通知内部并发 | 不阻塞触发操作；有界并发、队列、关闭清理；不得应用异步阻断/改写/审批 |
| 异步信息送达 | 活动轮次下一安全请求点注入；空闲时等下一用户轮次，不自动启动轮次，[C20][C21] | stdout 丢弃 | 异步结果不能回写正在发送的请求；重配置不丢弃在途任务 |
| 超时 | 通常600s；SessionEnd/Interrupt默认1s、上限3s；SessionEnd始终同步，[C18] | command默认30s/http15s，全都夹到100–120,000ms，[S01] | 单位明确且兼容旧timeout_ms；超时包含stdin写入，不只等待进程退出 |
| 进程清理 | 超时/取消杀Unix进程组或Windows进程树；正常成功允许明确脱离的助手继续，[C20] | 隔离了进程，但超时后丢弃wait_with_output，未见杀进程/kill_on_drop逻辑，[S01] | 挂起、子进程保留输出管道、取消均不留下孤立进程 |
| shell与平台 | 可配置shell；Windows专用commandWindows；会话cwd和工具stdin里的cwd可独立，[C04][C18][C20] | 使用平台shell、context.workdir；Web文案却固定sh -lc，[S01][S07] | 统一shell设置；Windows覆盖字段；从子目录启动也能定位脚本 |
| 环境 | 会话启动环境快照后叠加hook变量，过滤内部不可继承凭据；插件根和数据目录变量，[C20][C22] | 继承进程环境并追加SAI_* | 不传播内部执行凭据；会话环境行为明确；不在日志打印密钥 |
| 配置发现与合并 | 活跃config层旁的hooks.json与inline hooks、插件hooks并存；高层不替换低层；同层双形式合并并提示，[C02][C23] | AppConfig单一items列表，[S02] | 用户/项目文件与现有配置并存，明确重复配置行为 |
| 来源信任 | 非托管定义需审查与hash信任；改动后不执行；用户或sessionflags才可覆盖信任/开关，[C02][C23][C24] | 启用的items直接执行 | 项目文件不能给自己授予信任，变更脚本/定义后重新审查 |
| 托管策略 | 管理员要求可限制只执行托管hooks；必需hook加载失败会拒绝启动；托管hook不能由用户禁用，[C02][C23] | 无 | 若宣称完全对齐，需明确受管理来源与加载失败处理，不只是加字段 |
| 插件hooks | 发现、hash与插件内容校验；运行前重新检查内容，可避免发现后文件被替换，[C23] | hooks未接插件生命周期来源 | 与现有Sai插件权限/生命周期一致，不引入外部内核概念 |
| 运行状态与界面 | 预览、Started/Completed、source、scope、状态、时间、duration、输出entries、statusMessage，[C17][C21] | stderr简单打印；Web无运行状态 | CLI/Web能看到hook名称、事件、结果、耗时和错误；统一展示而非原生弹窗 |
| 输出预算 | 每条模型可见hook输出默认约2500tokens；超限保存全文并给首尾预览与回读路径；0表示不保存裁剪，[C25] | stdout全部丢弃，HTTP响应不解析 | 按handler计预算，正文保存失败仍安全裁剪，不能吞没上下文窗口 |
| 通知 | 旧notify命令把完成事件JSON追加为最后一个argv；新Stop异步命令可承载通知，[C26][C27] | 无等价完成payload，agent_end缺用户输入和最后回复 | 完成轮次后只通知一次；邮箱脚本可验证真实事件与正确payload |

## 邮箱验证方式

Codex 本地 hooks 没有内置 SMTP 邮件发送器，也没有名为 `Notification` 的上述生命周期事件。它提供的是执行通知脚本的机制，不能把“邮箱通知”当作 Codex 原生 SMTP 能力。[C03][C26]

两种已经证实的机制：

1. 旧 `notify = ["program", "args..."]`：轮次完成后向命令追加 JSON argv，类型为 `agent-turn-complete`，包含 thread-id、turn-id、cwd、input-messages、last-assistant-message。命令创建后不等待，输出不参与控制。[C26][C27]
2. 新 `Stop` 的 async command：通过 stdin 收到 Stop JSON，脚本读取 `last_assistant_message`、`turn_id` 等并自行发送邮件。异步输出不能改变停止决定。[C07][C20]

Sai 验证建议使用第二种，不为了发送邮件增加另一套内核或通知专用模型请求。示例是机制说明，调研阶段没有安装或执行：

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "uv run /home/snemc/.claude/skills/qq-mail/scripts/send_mail.py --subject 'Sai hooks 验证' -m - 1>&2",
            "async": true,
            "timeout": 30,
            "statusMessage": "发送完成通知"
          }
        ]
      }
    ]
  }
}
```

发送脚本采用用户已有 QQ Mail 技能：`smtp.qq.com:587`，STARTTLS，默认发件人与收件人均使用用户本地 QQ 邮箱。认证材料留在环境或既有私有配置，禁止写入仓库hooks模板、日志或报告。示例将发送脚本的诊断stdout重定向到stderr，让Stop观察到成功退出且空stdout，避免已经发送邮件却因非JSON日志被记为hook失败。可以先发送完整事件 JSON 以核对字段，再用小型适配脚本格式化最终回复；不建议把任意模型文本拼进shell命令。

验收应分开报告：hook选择与完成payload正确、脚本执行成功、SMTP接收成功。除非检查了收件箱，不能把SMTP接受直接写成邮件已进入收件箱。中断、失败、Stop要求继续时，不应误称最终完成；如要对失败也通知，需单独配置明确事件与状态。

## 必须覆盖的验证

- 14事件的真实调用点；尤其是模型重试、失败、取消、手动/自动/后台摘要应用、主会话启动/恢复/清空、子代理启动/完成。
- PreToolUse拒绝时真实工具没有运行；改写后使用新参数且仍经过原安全门禁；并发工具与代码模式不能绕过。
- PermissionRequest的deny优先、无决定保留审批；不支持字段不能自动放行。
- PostToolUse副作用保留、原结果被反馈替换；代码模式promise语义正确。
- Stop第一次与已经续接的stop_hook_active；continue:false优先；空reason或无效JSON不能形成静默死循环。
- 同步handler并发启动、多别名只执行一次、最终改写选择规则、异步并发上限与队列、不自动产生用户轮次。
- 超时期间stdin写入、进程树清理、成功后分离子进程、会话关闭与重配置。
- hook输出超限保存与裁剪、失败回退；结构化stdout和诊断stderr分离。
- 全局/项目来源合并、信任hash变更、托管不能自禁、必需hook加载失败。
- Web表单事件/类型/字段与Rust契约一致，旧配置迁移后仍可编辑。
- 邮件采用mock SMTP测试payload与失败路径，再通过真实Stop hook发送一次测试邮件；调研不替代这项执行验证。

Codex 已有对应测试，可供直接对照：hooks crate的事件解析测试、`engine/command_runner_tests.rs`、`engine/mcp_runner_tests.rs`，以及 core 集成测试 `hooks.rs`、`hooks_cwd_tests.rs`、`hooks_mcp.rs`、`hooks_plugin_policy_refresh.rs`、`interrupt_hooks.rs`。[C21][C28] Sai 现有 `tool_batch_execution_tests.rs` 的HookContext只用于工具批处理夹具，不能视为上述控制协议测试。[S09]

## 本次功能清理的边界

- 新 hooks 调用点应落在 Sai 自带 agent、工具执行、会话和压缩模块；ACP/外部多内核应直接清理，不作为 hooks 补全依赖。[S03][S04][S05]
- 用户可见子代理的生命周期独立于外部内核，删除多内核不等于删除子代理能力；SubagentStart/Stop仍可在自带子代理管理层接入。
- 内置人格不属于 hooks 功能。不要将人格提示重新包装成隐式SessionStart hook以规避用户的清理要求。
- 原有 HTTP hook 是 Sai 自有扩展，Codex没有同款直接HTTP handler；对齐 command/mcp_tool 不要求删除正常HTTP用户配置。

## 来源索引

Codex 相对路径均以 `/home/snemc/workspace/codex/` 为根，Sai相对路径以本仓库为根；行号对应基准源码。

- [C01] `codex-rs/features/src/lib.rs:1512`：hooks稳定状态与默认开关。
- [C02] `codex-rs/hooks/src/engine/discovery.rs:1000`、`codex-rs/hooks/src/config_rules.rs:15`：来源信任与用户覆盖规则。
- [C03] `codex-rs/hooks/src/lib.rs:29`：14事件与9类有意义的matcher。
- [C04] `codex-rs/hooks/src/events/pre_tool_use.rs:24`、`:119`、`:153`、`:181`：前置工具协议、决定聚合、改写选择。
- [C05] `codex-rs/hooks/src/events/permission_request.rs:91`、`:153`、`:197`：审批决定与失败处理。
- [C06] `codex-rs/hooks/src/events/post_tool_use.rs:24`、`:186`；官方Hooks页PostToolUse/代码模式段。
- [C07] `codex-rs/hooks/src/events/stop.rs:24`、`:251`、`:330`：Stop/SubagentStop输出解析与继续执行。
- [C08] `codex-rs/hooks/src/events/compact.rs:24`、`:268`：PreCompact/PostCompact。
- [C09] `codex-rs/hooks/src/events/session_start.rs:24`、`:223`：开始来源及子代理限制。
- [C10] `codex-rs/hooks/src/events/model_request.rs:1`、`:19`、`:66`：请求观察事件与状态/用量。
- [C11] `codex-rs/core/src/hook_runtime.rs:375`：主/子代理Stop选择、stop_hook_active及记录路径。
- [C12] `codex-rs/hooks/src/events/session_end.rs:20`、`:50`、`:96`；`codex-rs/core/src/hook_runtime.rs:455`：收尾行为。
- [C13] `codex-rs/hooks/src/events/interrupt.rs:24`、`:117`；`codex-rs/core/src/hook_runtime.rs:499`：中断行为。
- [C14] `codex-rs/hooks/src/schema.rs:89`、`:277`、`:428`、`:587`：事件专用输入与输出类型。
- [C15] `codex-rs/hooks/src/engine/output_parser.rs:345`、`:369`、`:401`、`:441`：JSON对象要求及不支持字段。
- [C16] `codex-rs/hooks/src/engine/matcher.rs:11`：精确候选与正则匹配。
- [C17] `codex-rs/hooks/src/engine/dispatcher.rs:41`、`:123`、`:243`：别名去重、同步并发与运行状态。
- [C18] `codex-rs/hooks/src/engine/discovery.rs:651`、`:729`、`:782`、`:946`：handler、平台覆盖、异步例外、超时。
- [C19] `codex-rs/hooks/src/engine/mcp_runner.rs:29`、`:107`：MCP调用和类型保持模板。
- [C20] `codex-rs/hooks/src/engine/command_runner.rs:54`、`:119`、`:174`、`:190`、`:314`、`:383`、`:444`：队列、输出、进程与环境。
- [C21] `codex-rs/core/tests/suite/hooks.rs:1760`、`:1877`、`:2032`、`:2260`、`:2557`：异步安全注入、输出保存、压缩续接与Stop续接测试。
- [C22] `codex-rs/hooks/src/engine/discovery.rs:312`：插件根和数据目录变量。
- [C23] `codex-rs/hooks/src/engine/discovery.rs:118`、`:390`、`:811`、`:917`：分层发现、定义与内容hash、准入。
- [C24] `codex-rs/hooks/src/config_rules.rs:23`：只有用户与sessionflags覆盖信任/启用状态。
- [C25] `codex-rs/hooks/src/output_spill.rs:12`、`:45`、`:65`、`:122`：2500token阈值、全文保存和预览。
- [C26] `codex-rs/hooks/src/legacy_notify.rs:13`、`:45`：旧notify的argv协议和异步创建进程。
- [C27] `codex-rs/core/src/hook_runtime.rs:621`、`:646`；`codex-rs/core/src/config/mod.rs:807`：完成通知输入与配置。
- [C28] `codex-rs/hooks/src/engine/command_runner_tests.rs:543`、`:574`、`:628`、`:685`、`:751`；`codex-rs/hooks/src/engine/mcp_runner_tests.rs:92`：并发、恢复、取消、进程组与MCP协议测试。
- [S01] `src/hooks/mod.rs:11`、`:47`、`:69`、`:112`：八事件、分发、command/http执行。
- [S02] `src/config/model.rs:555`、`:564`；`src/config/app_config.rs:87`：配置结构。
- [S03] `src/agent/turn_tools.rs:47`、`:104`、`:196`、`:523`：现有事件时机。
- [S04] `src/agent/tool_batch_execution.rs:128`、`:219`：并发工具独立事件路径。
- [S05] `src/agent/codemode_dispatch.rs:47`、`src/agent/codemode_readonly.rs:44`：代码模式路径。
- [S06] `web/src/features/settings/hooks-settings-section.tsx:21`、`:46`、`:56`：当前Web设置行为。
- [S07] `web/src/features/settings/hooks/hook-action-fields.tsx:13`：命令/HTTP字段和shell文案。
- [S08] `web/src/features/settings/hooks/hook-templates.ts:4`、`:12`、`:23`；`web/src/api/contracts/config.ts:426`：事件、模板与契约。
- [S09] `src/agent/tool_batch_execution_tests.rs:98`、`:188`：HookContext夹具用途。
- [D01] https://developers.openai.com/codex/hooks：发布行为与尚未支持功能的官方说明。
- [D02] https://developers.openai.com/codex/config-reference：features.hooks、notify和托管限制。

检索命令：先检查知识图谱工具可用性；不可用后运行带 `--key-env` 的Fast Context进行代码定位，再在已知文件范围内精确读取。网络使用 `smart-search doctor --format json`、`smart-search exa-search 'site:developers.openai.com/codex hooks PreToolUse UserPromptSubmit Stop notify' --num-results 5 --format json` 与两次 `smart-search fetch`。主搜索供应商doctor报告TLS错误，Exa与Tavily可用；调研未使用不可信综合生成结果作为事实来源。
