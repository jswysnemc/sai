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

传输重试属于同一逻辑模型请求，不会重复产生开始事件。工具执行独立于模型请求范围。主 Agent、子任务以及各工具入口共用相同分发语义；外部对话内核提供 Agent 级事件，不伪造无法观察的内部模型请求事件。

`tool_call` 只接受 `nil` 或 `{deny="原因"}`；拒绝原因必须非空且不超过 4096 字节。异常或非法返回值阻止本次工具执行。监听器不能批准宿主已拒绝的操作，也不能修改真实参数。

其他事件仅用于观察；返回值不会注入模型消息。监听器失败独立诊断，不替换业务结果，也不阻止其他插件。事件回调没有写入授权。

外部取消会回收正在运行的 Future，不另外启动后台任务补发结束事件。插件不能依赖结束监听器释放宿主资源；取消、I/O 回收和实例释放由 Rust 负责。

## 上下文与状态

| 字段或函数 | 含义 |
| --- | --- |
| `sai.plugin_id` | 当前插件 ID |
| `sai.config` | 当前包设置的快照 |
| `ctx.session_id` | 宿主提供的会话标识；子任务使用独立标识 |
| `ctx.workdir` | 本次调用所属任务的真实工作目录 |
| `ctx.progress(text)` | 单条最多 4096 字节，每次调用最多 128 条 |

模型参数无法覆盖这些宿主字段。保存旧 `ctx.progress` 后在后续调用使用会报错，避免向已经完成的工具写入进度。

同一 Agent 的工具表副本、工具、命令和监听器共享 VM。新 Agent、新会话和子任务创建独立 VM；重新加载时，源码、设置和授权全部未变才沿用原实例。注册元数据必须可重复生成。

Lua 全局变量属于当前实例，不是持久会话存储。进程重启、创建新 Agent 或切换会话都会重新初始化；不同交互入口对 Agent 的复用时长可能不同。

## 宿主能力

### HTTP

```lua
local response = sai.http.request({
    url = "https://api.example.com/items",
    method = "GET",
    headers = { accept = "application/json" },
    max_bytes = 65536,
})
```

响应包含 `status`、`headers`、`text`。支持 GET、HEAD、POST、PUT、PATCH、DELETE；GET/HEAD 以外的方法还要求当前工具或命令声明 `writes`，并通过 Sai 授权。HTTP 错误状态作为响应返回，由业务代码选择处理方式。

初始 URL 和每次重定向都必须属于有效来源集合；URL 不允许凭据，最多跟随 5 次重定向。Host、Connection、Content-Length、Transfer-Encoding 和代理授权头由宿主控制。最多 32 个请求头，总计 16 KiB；URL 最多 8192 字节。

HTTP 请求本身最多 30 秒，同时受回调总时长约束。正文和响应都有字节上限，`max_bytes` 不得突破包的 `output_bytes`。宿主按 Content-Type 的 charset 解码文本，并再次检查解码后的大小。

### JSON、文本与时间

| 接口 | 行为 |
| --- | --- |
| `sai.json.decode(text)` | JSON 转 Lua 值 |
| `sai.json.encode(value)` | Lua 值转 JSON 文本 |
| `sai.json.array(table?)` | 保留空数组的类型 |
| `sai.json.null` | 表示 JSON null |
| `sai.text.url_encode(text)` | URL 百分号编码 |
| `sai.text.html_to_text(html, width?)` | HTML 转文本，宽度默认 120，范围 20–200 |
| `sai.text.clip(text, count)` | 按 Unicode 字符截取并附截断说明 |
| `sai.time.now()` | 当前 Unix 秒时间戳 |
| `sai.time.iso(seconds, offset_seconds?)` | 指定时区的 ISO 时间，默认 UTC |

运行时提供 Lua 5.4 的表、字符串、数学和 UTF-8 标准库。没有 `io`、`os`、`package`、`debug`、`load`、`loadfile`、`dofile`、直接 `coroutine` 或原生动态库入口。HTTP 仅在回调执行期间可用。

## 资源范围

| 限制 | 默认值 | 可配置范围 |
| --- | --- | --- |
| Lua 堆内存 | 16 MiB | 1–64 MiB |
| 单次指令预算 | 2000000 | 1000–20000000 |
| 回调总时长 | 20 秒 | 0.1–60 秒 |
| 输出与 HTTP 大小 | 1 MiB | 1 KiB–4 MiB |

Lua 计算位于阻塞工作线程，受指令 Hook、堆内存和总时长约束。取消会停止受管宿主 I/O，并在 Lua 恢复执行时终止回调。能力隔离不等于操作系统进程隔离；这里的内存上限针对 Lua 堆，不是整个 Sai 进程。

版本 1 仅提供 HTTP、JSON、文本和时间能力。文件、进程、持久插件存储、模型上下文变换和界面组件扩展尚未开放，需要独立契约与授权设计。
