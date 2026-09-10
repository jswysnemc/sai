# Lua API v1

## 包格式

```text
my-plugin/
├── sai-plugin.json
├── init.lua
└── format.lua
```

```json
{
  "api_version": 1,
  "id": "my-plugin",
  "version": "1.0.0",
  "name": "示例插件",
  "description": "查询外部服务并提供用户命令",
  "entry": "init.lua",
  "capabilities": { "http": ["https://api.example.com"] },
  "limits": {
    "memory_bytes": 16777216,
    "instructions": 2000000,
    "timeout_ms": 20000,
    "output_bytes": 1048576
  }
}
```

插件 ID 以小写字母开头，只允许小写字母、数字、短横线和下划线，最多 32 字节；版本使用 SemVer。清单拒绝未知字段。

入口和模块必须是包内规范相对 `.lua` 路径。拒绝符号链接、绝对路径、父目录跳转以及无法在 Windows 安装的设备保留名和文件名。最多 128 个 Lua 文件、4 MiB 源码、12 层目录；清单最多 64 KiB。

`require("format")` 加载 `format.lua`；`require("feature.format")` 加载 `feature/format.lua`。源码在加载时形成快照，之后 `require` 不会读取磁盘上修改过的文件。禁止原生库和包外模块。

## 注册工具

```lua
--- 【示例插件】【查询】读取指定条目并返回服务响应
--- @param args table 包含 id 字段
--- @param ctx table 宿主提供的会话信息和进度函数
--- @return table 查询结果
local function lookup(args, ctx)
    ctx.progress("正在查询")
    local response = sai.http.request({
        url = "https://api.example.com/items/" .. sai.text.url_encode(args.id),
        headers = { accept = "application/json" },
    })
    assert(response.status == 200, "service returned HTTP " .. response.status)
    return sai.json.decode(response.text)
end

sai.register_tool({
    name = "lookup",
    description = "Read an item from the configured service.",
    parameters = {
        type = "object",
        properties = { id = { type = "string", minLength = 1 } },
        required = { "id" },
        additionalProperties = false,
    },
    access = "read_only",
    execute = lookup,
})
```

`access` 允许 `read_only`（默认）和 `writes`。参数必须是对象 JSON Schema；本地校验发生在业务函数和 HTTP 调用之前。Schema 最多 64 KiB，仅允许本地 `$ref`。工具和命令包内名称最多 48 字节；外部工具最终名称 `lua__<id>__<name>` 不得超过 64 字节，超长时拒绝整个包，不截断名称。

返回字符串时直接作为工具文本；其他可序列化值输出为 JSON。`nil` 返回空文本。定义、说明和参数直接来自插件，不被旧工具说明表覆盖。

## 注册用户命令

```lua
--- 【示例插件】【状态】返回当前插件自己的设置摘要
--- @param arguments string 用户提供的完整参数文本
--- @param ctx table 当前会话上下文
--- @return string 展示文本
local function status(arguments, ctx)
    return "session=" .. ctx.session_id
end

sai.register_command({
    name = "status",
    description = "Show plugin status.",
    access = "read_only",
    execute = status,
})
```

CLI 使用 `sai plugins run my-plugin status`，TUI 使用 `/plugin my-plugin/status`。命令不向供应商注册为模型工具，执行仍经过 Sai 的权限检查、审计和插件工具检查。写入命令不能在计划模式下执行。

注册仅允许在入口及其同步 `require` 期间进行。一个包最多注册 128 个工具、命令和事件监听器；重复名称、无效契约或初始化失败会撤销整个加载事务。

## 生命周期

```lua
--- 【示例插件】【执行检查】按项目约束收窄工具权限
--- @param event table 含 name 和 arguments
--- @param ctx table 只读宿主上下文
--- @return table|nil 拒绝原因，或 nil 表示继续宿主检查结果
local function check_tool(event, ctx)
    if event.name == "write_file" and event.arguments.path == "protected.txt" then
        return { deny = "protected.txt 由项目维护流程更新" }
    end
end

sai.on("tool_call", check_tool)
```

| 事件 | 触发时机 | 数据 |
| --- | --- | --- |
| `agent_start` | 一个用户请求或子任务段开始 | 主 Agent 有 `turn_id`；子任务有 `kind` |
| `agent_end` | 请求返回成功或错误 | 开始数据及 `ok` |
| `turn_start` | 一个逻辑模型请求开始 | `round`；主 Agent 另有 `turn_id` |
| `message_start` | 同一模型请求开始，位于 `turn_start` 之后 | 同上 |
| `message_end` | 模型请求结束 | 同上及 `ok` |
| `turn_end` | `message_end` 之后 | 同上及 `ok` |
| `tool_call` | 实际工具通过宿主授权，尚未执行 | `name`、原始 `arguments` |
| `tool_result` | 工具执行或插件检查结束 | `name`、`ok`、最多 16384 字符的 `output` |
| `reply_end` | 交互面为一次结束状态计算通知 | `surface`、`status`、`locale`；使用下述独立纯运行时 |

传输重试属于同一逻辑模型请求，不会重复产生开始事件。工具执行独立于模型请求范围。主 Agent、子任务以及各工具入口共用相同分发语义；外部对话内核提供 Agent 级事件，不伪造无法观察的内部模型请求事件。

`tool_call` 只接受 `nil` 或 `{deny="原因"}`；拒绝原因必须非空且不超过 4096 字节。异常或非法返回值阻止本次工具执行。监听器不能批准宿主已拒绝的操作，也不能修改真实参数。

除 `tool_call` 和 `reply_end` 外，事件仅用于观察；返回值不会注入模型消息。监听器失败独立诊断，不替换业务结果，也不阻止其他插件。事件回调没有写入授权，也没有模型和工具调用服务，不能沿用上一工具回调的权限。

外部取消会回收正在运行的 Future，不另外启动后台任务补发结束事件。插件不能依赖结束监听器释放宿主资源；取消、I/O 回收和实例释放由 Rust 负责。

### 通知纯回调

通知包声明 `"capabilities": {"notifications": true}`。外部包还需要用户通过 `sai plugins enable <id> --allow-notifications` 授权；`--no-notifications` 只撤销这一项，其他分项更新也不会改变通知授权。

```lua
--- 【通知示例】【完成提醒】为 Web 完成状态提供纯展示数据
--- @param event table 包含交互面、结束状态与区域代码
--- @return table|nil 通知数据，不适用时返回 nil
local function notice(event)
    if event.surface ~= "web" or event.status ~= "completed" then return nil end
    return {title="Sai", body="Reply complete", desktop=true, sound=false}
end
sai.on("reply_end", notice)
```

`surface` 只接受 `tui`、`web`；`status` 只接受 `completed`、`interrupted`、`failed`。`locale` 是最多 32 字节的区域代码，当前交互面使用 `en-US` 或 `zh-CN`。不会传入回复正文、工作目录或会话私有状态，CLI 单次执行与子任务没有这个展示入口。

每次计算重新读取已保存的插件设置、授权和源码，建立独立纯 VM。不要依赖 Agent 的 Lua 全局变量、事件配对或连接内状态。此 VM 只获得通知授权，不提供网络、文件、进程、模型、工具调用或存储服务；同一包在普通工具回调中的其他授权不参与通知计算。返回值只交给交互面的投递器，不进入模型上下文，也不产生后台命令或自动续聊。

一个监听器返回 `nil` 或 `{title, body, desktop, sound}`，四个字段必须类型正确，不接受额外字段。标题须非空且最多 256 字节，正文最多 4096 字节；拒绝换行、回车和制表符之外的控制字符。正文的整理与截断属于 Lua 策略。一个包出现任何非法返回值时整批丢弃，后续包继续执行。

展示实例的 Lua 堆最多 4 MiB、指令最多 100,000、加载及回调各使用 100 毫秒时限、序列化结果最多 16 KiB；更小的清单预算继续有效。一次计划最多处理 16 个策略、交付 8 条通知，总等待预算两秒。发现与加载在阻塞工作线程中执行；文件读取的取消边界沿用系统接口约束。

TUI 使用同一纯策略入口并在后台线程完成系统投递。Web 通过已认证的 `POST /api/notifications/plan` 提交 `{"status":"completed","locale":"zh-CN"}`，取得禁止缓存的 `{notifications, diagnostics}`；该接口本身不在服务器桌面显示通知。浏览器使用 SSE 封套中的 `replayed` 字段区分历史恢复，按工作区、会话和运行标识消费通知。已知活动运行在断线期间完成时，补发仍可以通知一次；会话切换会取消尚未完成的请求及权限回调。

## 上下文与状态

| 字段或函数 | 含义 |
| --- | --- |
| `sai.plugin_id` | 当前插件 ID |
| `sai.config` | 当前包设置的快照 |
| `sai.limits` | 清单资源限制的 Lua 副本；修改它不能提高 Rust 实际限制 |
| `ctx.session_id` | 宿主提供的会话标识；子任务使用独立标识 |
| `ctx.operation_id` | 同一次用户操作及其插件组合调用共享的标识 |
| `ctx.allow_writes` | 本次工具或命令实际取得的写入权限；事件为 false |
| `ctx.workdir` | 本次调用所属任务的真实工作目录 |
| `ctx.progress(text)` | 单条最多 4096 字节，每次调用最多 128 条 |

模型参数无法覆盖这些宿主字段。保存旧 `ctx.progress` 后在后续调用使用会报错，避免向已经完成的工具写入进度。

同一 Agent 的工具表副本、工具、命令和监听器共享 VM。新 Agent、新会话和子任务创建独立 VM；重新加载时，源码、设置和授权全部未变才沿用原实例。注册元数据必须可重复生成。

需要模型或工具服务的插件调用使用独立的嵌套执行环境：完整复制当前 Agent 的工具目录并重新建立插件 VM，再按调用插件的声明与授权限制可见名称。嵌套事件不会进入正在执行的 Lua VM；原生包装器持有旧注册表时，宿主同样隔离活动监听器。调用完成、失败或取消都会撤销本次服务，不在长期存活的 VM 中保留注册表引用。

Lua 全局变量属于当前实例，不是持久会话存储。进程重启、创建新 Agent 或切换会话都会重新初始化；不同交互入口对 Agent 的复用时长可能不同。

## 宿主能力

### 文件、环境与进程

`sai.system` 提供平台和宿主 PID；`sai.env.get`、`sai.fs.read_text/read_dir/stat` 和 `sai.process.output` 分别使用精确环境授权、路径授权和固定进程模板。详细参数、返回值、取消边界及授权示例见[系统接口](system-api.md)。这些 I/O 接口仅在回调中开放，工作目录和权限由 Rust 调用状态提供。

### 模型与工具

模型与工具调用分别声明、分别授权：

```json
{
  "capabilities": {
    "model": true,
    "tools": ["read_file", "web_search"]
  },
  "limits": {
    "model_requests": 32,
    "tool_calls": 128
  }
}
```

`tools` 只接受精确名称，最多 128 项；每项最多 64 字节，仅包含 ASCII 字母、数字、`_` 和 `-`。外部插件使用完整的 `lua__<id>__<tool>` 名称。有效范围同时受清单、用户授权、当前 Agent 工具白名单和工具或命令的读写权限约束。

```lua
local available = sai.tools.list()
local response = sai.model.complete({
    messages = {
        { role = "system", content = "根据已取得的资料回答问题。" },
        { role = "user", content = "说明这段资料。" },
    },
    tools = sai.json.array({ "read_file" }),
    stream_reasoning = false,
    timeout_ms = 10000,
})
```

`sai.model.complete` 只完成一次请求。消息支持 `system`、`user`、`assistant`，包含文本 `content` 和可选文本 `reasoning`；最多 256 条，最后一条必须是 `user`。不自动加入主对话、系统提示或用户消息。工具列表最多 128 项，省略或空数组表示此次不提供工具。未知请求字段会被拒绝，不能传入供应商、地址、模型名称或 API Key。

请求使用当前 Agent 或子任务实际选定的客户端；模型切换、重载和工具表替换不会沿用旧配置来源。独立 CLI 命令在首次模型请求时才解析当前配置，管理和纯工具命令不需要初始化模型。供应商与凭据只存在于 Rust 宿主。

返回值包含以下字段：

| 字段 | 内容 |
| --- | --- |
| `content` | 模型正文 |
| `reasoning` | 可选思考文本，缺失时为 JSON null |
| `tool_calls` | `{id, name, arguments}` 列表；`arguments` 保留为 JSON 文本 |
| `usage` | 可选单次请求用量：`prompt_tokens`、`completion_tokens`、`total_tokens`、`cache_read_tokens`、`cache_write_tokens` |

宿主不会自动执行模型建议。插件可用 `sai.tools.call(name, arguments, options?)` 显式调用工具；参数接受 Lua 对象表或原始 JSON 对象文本，返回工具文本。该入口沿用正常参数解析、权限、插件检查和审计，运行中切换到计划模式也会阻止后续写入。`pcall` 可以捕获调用错误，嵌套错误保留具体原因。

第三个参数可省略，也可传入 `{timeout_ms=5000}`，只接受可选的非负整数 `timeout_ms` 字段。缺省时使用回调总时长，实际时限限制在 1 毫秒至回调上限；单次设置不能延长整个回调的截止时间。超时会释放正在等待的宿主 Future，Lua 可以捕获错误并继续调用其他工具；该次请求仍计入调用预算。未知选项、错误类型和负数在执行工具与消耗预算之前被拒绝，选项不能覆盖工作目录或授权。

`sai.tools.list()` 返回当前可调用的 `{name, display_name, description, parameters, access}` 列表。模型目录和工具执行使用同一授权范围。供应商公开的 `sai_web_search` 别名会恢复为 `web_search`；未公开的内部执行别名不会转换成已授权工具。

如果 A 声明调用 B，B 可以使用自己已授权的 C，但 A 不能直接调用 C。所有层级仍受原 Agent 工具白名单约束。目录排除当前插件和祖先插件；再次进入祖先插件或超过 8 层会在事件分发之前失败。每次需要模型或工具能力的调用都有独立的 `agent_start/end`，每次模型请求都有正常的轮次和消息事件。

模型请求 `timeout_ms` 默认使用回调总时长，限制在 1 毫秒至该回调上限；请求超时可由 `pcall` 捕获。模型请求和结果、工具目录、参数及输出受 `output_bytes` 约束，模型正文、思考和工具参数在流式接收时也检查大小。失败的实际模型或工具请求同样消耗次数预算。业务循环应为最终报告预留模型、工具和消息数量空间。

### HTTP

```lua
local response = sai.http.request({
    url = "https://api.example.com/items",
    method = "GET",
    headers = { accept = "application/json" },
    max_bytes = 65536,
    timeout_ms = 12000,
})
```

响应包含 `status`、`headers`、`text`。支持 GET、HEAD、POST、PUT、PATCH、DELETE。GET/HEAD 允许在只读回调中使用；其他方法默认要求当前工具或命令声明 `writes`，并通过 Sai 授权。HTTP 错误状态作为响应返回，由业务代码选择处理方式。

搜索等使用 POST 的只读接口可另外声明精确端点：

```json
{
  "capabilities": {
    "http": ["https://api.example.com"],
    "http_read_only_post": ["https://api.example.com/search"]
  }
}
```

来源与 `http_read_only_post` 都必须获得授权。端点使用规范 HTTP(S) 地址，包含精确路径，不允许查询参数、片段或内嵌凭据；最多 32 个端点。端点授权允许查询参数，不覆盖子路径，也不允许 PUT、PATCH 或 DELETE。该声明表示插件对接口查询用途的契约，插件仍须保证 POST 内容为只读操作。

初始 URL 和每次重定向都使用同一授权检查。保留 POST 的重定向仍须匹配查询端点；303 等按 HTTP 语义改为 GET 后，宿主移除原正文。最多跟随 5 次重定向，整个过程共用同一截止时间。跨来源跳转不转发正文，只保留 Accept、Accept-Language、User-Agent，避免泄露标准或自定义认证头。Host、Connection、Content-Length、Transfer-Encoding 和代理授权头由宿主控制。最多 32 个请求头，总计 16 KiB；URL 最多 8192 字节。

请求 `timeout_ms` 默认 30,000 毫秒，运行时将其限制为 1 至清单的 `limits.http_timeout_ms`，该清单值默认同样为 30,000，硬上限为 120,000。请求超时会取消受管 I/O，并作为 Lua 错误交给 `pcall`，便于保留静态规则或已有结果；整个回调仍受清单总时长约束。正文和响应都有字节上限，`max_bytes` 不得突破包的 `output_bytes`。宿主按 Content-Type 的 charset 解码文本，并再次检查解码后的大小。

### JSON、文本与时间

| 接口 | 行为 |
| --- | --- |
| `sai.json.decode(text)` | JSON 转 Lua 值 |
| `sai.json.encode(value)` | Lua 值转 JSON 文本 |
| `sai.json.array(table?)` | 保留空数组的类型 |
| `sai.json.null` | 表示 JSON null |
| `sai.text.trim(text)` | 去除两端 Unicode 空白，保留正文内容 |
| `sai.text.upper(text)` | 只接受字符串，按 Unicode 规则转为大写；输入及展开后的 UTF-8 字节数均受 `output_bytes` 限制 |
| `sai.text.number_to_string(number)` | 只接受有限数值，以 f64 十进制格式保留有效数字及负零，不使用指数形式；整数先按 f64 语义转换 |
| `sai.text.collapse_whitespace(text)` | 去除首尾空白，并把连续 Unicode 空白替换为一个空格；输入受 `output_bytes` 限制 |
| `sai.text.estimate_tokens(text)` | 使用 Sai 分词器估算文本 token 数量；输入受 `output_bytes` 限制 |
| `sai.text.url_encode(text)` | URL 百分号编码 |
| `sai.text.html_to_text(html, width?)` | HTML 转文本，宽度默认 120，范围 20–200 |
| `sai.text.html_to_markdown(html)` | HTML 转 Markdown，保留标题、链接、列表和代码格式 |
| `sai.text.clip(text, count)` | 按 Unicode 字符截取并附截断说明 |
| `sai.time.now()` | 当前 Unix 秒时间戳 |
| `sai.time.iso(seconds, offset_seconds?)` | 指定时区的 ISO 时间，默认 UTC |

HTML 和 Unicode 大写转换前后的 UTF-8 文本均受包内 `output_bytes` 限制。Markdown 转换复用 `html2md`，相对链接保持原地址；正文范围提取与业务截断由插件负责。最终回调结果另受包含 JSON 封装在内的输出预算约束。

`number_to_string` 不会把数字字符串、布尔值或 null 转成数值，也拒绝 Lua 计算产生的 NaN 和无穷大。它用于与宿主 f64 展示保持一致，例如 `1e-20` 返回 `0.00000000000000000001`；超过 f64 精确整数范围的值可能舍入。

运行时提供 Lua 5.4 的表、字符串、数学和 UTF-8 标准库。没有 `io`、`os`、`package`、`debug`、`load`、`loadfile`、`dofile`、直接 `coroutine` 或原生动态库入口。HTTP 仅在回调执行期间可用。

## 资源范围

| 限制 | 默认值 | 可配置范围 |
| --- | --- | --- |
| Lua 堆内存 | 16 MiB | 1–64 MiB |
| 单次指令预算 | 2000000 | 1000–20000000 |
| 回调总时长 | 20 秒 | 0.1–3600 秒 |
| 单次 HTTP 最大时长 | 30 秒 | 0.001–120 秒 |
| 输出与宿主结果大小 | 1 MiB | 1 KiB–4 MiB |
| 单次回调模型请求数 | 32 | 1–256 |
| 单次回调工具调用数 | 128 | 1–1024 |
| 单次回调系统调用数 | 1024 | 1–4096 |

Lua 计算位于阻塞工作线程，受指令 Hook、堆内存和总时长约束。取消释放受管 HTTP、模型、工具和模板进程 Future，并在 Lua 恢复执行时终止回调；已开始的阻塞文件操作不保证强制中断。能力隔离不等于操作系统进程隔离；这里的内存上限针对 Lua 堆，不是整个 Sai 进程。

长查询必须在清单中显式提高限制，不改变其他插件的默认值。`web-search` 保留旧版单个供应商 1–120 秒的配置范围，包的回调上限为 750 秒，覆盖六个供应商依次回退。

`linux-game-investigation` 和 `input-method-investigation` 均声明 32 MiB Lua 堆、2000 万指令、900 秒、2 MiB 输出、256 次模型请求和 1024 次工具调用。调查工具的业务步数及单工具超时可以进一步收窄，但不能突破这些宿主限制。

`diagnostic-evidence` 声明 32 MiB Lua 堆、2000 万指令、300 秒、2 MiB 输出、2048 次系统调用及 4 次工具调用。包设置继续收窄单次命令时长和输出字符数。

三个查询包采用各自的 HTTP 与回调上限，其他资源沿用运行时默认值：

| 包 | 单次 HTTP | 回调总时长 | 单次正文上限 |
| --- | --- | --- | --- |
| `weather` | 30 秒 | 35 秒 | 64 KiB |
| `exchange-rate` | 30 秒 | 65 秒 | 1 MiB |
| `moegirl` | 10 秒 | 45 秒 | REST 页面 512 KiB，搜索与解析 API 1 MiB |

版本 1 提供 HTTP、单次模型请求、显式工具调用、受限文件与环境访问、模板进程、JSON、文本和时间能力。私有会话存储、有界归档和工作目录接口见[私有状态与工作目录](private-api.md)。主 Agent 模型上下文变换和界面组件扩展尚未开放。
