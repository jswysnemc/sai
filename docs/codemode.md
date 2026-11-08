# code-model 模式

code-model 是会话模式，通过 `/code-model` 或 Shift+Tab 启用。
单轮任务可使用 `sai --code-model ask "任务内容"`。
Shift+Tab 按 YOLO → AUDIT → AUTO-AUDIT → CODE-MODEL → YOLO 循环；计划模式需显式退出。
切换在输入期间和模型执行期间均可使用，下一次请求使用新的工具声明。
普通模式不暴露脚本入口，工具设置也不将它列入任何分组。

此模式用 JavaScript 编排 Sai 工具。模型通过内部执行入口 `codemode` 传入
`{"code":"..."}`；脚本作为异步函数体执行，支持顶层 `await` 和 `return`。
它需要当前 Agent 的执行桥接，不能通过独立 `__tool` 命令执行。

## 配置

在 `config.jsonc` 中配置：

```json
{
  "codemode": {
    "inline_budget": 3000,
    "timeout_ms": 60000
  }
}
```

- 会话模式决定是否启用脚本入口，旧配置中的 `mode` 字段仍可读取，但不再启用普通模式下的工具。
- code-model 中模型只接收脚本入口；直接工具及调用网关也在执行门禁处拒绝。
- `inline_budget`：内联工具声明的估算 token 预算，最大 32000。
- `timeout_ms`：整个脚本的硬超时，包含启动、工具执行及审批等待，范围为 1–600000。

脚本入口由会话模式提供，无需加入 `enabled_tools`。嵌套工具仍受当前档案白名单约束，
脚本不能访问过滤后的注册表之外的工具。code-model 使用人工权限审核和工作区沙箱，
写入、外部路径与沙箱提升沿用既有审批流程。

## 脚本接口

```javascript
const results = await Promise.all([
  tools.read_file({path: "src/main.rs"}),
  tools.read_file({path: "Cargo.toml"})
]);
return results.map(result => String(result).split("\n").slice(0, 5));
```

- `tools.<name>(args)`：真实工具调用；特殊名称也可用 `tools["name"](args)`。
- 工具结果能解析为 JSON 时返回 JSON，否则返回文本。包含图片的结果返回
  `{result, images}`，图片通过 `image(result.images[0])` 显式输出。
- `text(value)`、顶层 `return value`：输出文本或 JSON。
- `image(value)`：输出 base64 PNG/JPEG/GIF/WebP data URL、`{image_url}` 或
  `{type: "image", data, mimeType}`；不接受远程图片 URL。
- `console.log/info/error/debug(...)`：输出控制台文本。
- `exit()`：结束脚本并提交成功状态。
- `ALL_TOOLS`：所有允许嵌套调用的工具名称和描述。
- `await searchTools(query, {limit, namespace})`：按名称和描述匹配工具，默认返回 8 项。
- `await describeTool(name)`：读取完整参数 Schema 和类型声明。
- `store(key, value)` / `load(key)`：存取分支内 JSON 状态；保存 `undefined` 删除键，
  保存 `null` 不删除键。单值最大 256 KiB，整个状态最大 1 MiB。

脚本可在第一行缩小输出或超时预算：

```javascript
// @options: {"max_output_tokens": 2000, "timeout_ms": 10000}
text(await tools.check_os_info({}));
```

脚本选项不能延长配置中的硬超时。默认输出预算为 10000 个估算 token。
超出预算的文本保留首尾，完整内容保存在会话的 `tool-results` 目录；结果包含路径。

## 执行边界

每次脚本创建独立原生 QuickJS 子进程，结束或取消后回收。JavaScript 没有
文件、网络、Node.js 或定时器 API，内置对象和工具桥接经过冻结保护。
工作进程清空环境变量；VM 内存上限为 256 MiB，栈上限为 1 MiB。
脚本最多 1 MiB，输出最多 16 MiB / 100000 项，工具调用最多 1024 次，
同时未完成请求最多 128 个。单个 IPC 帧最多 20 MiB，每次执行单向通信总量最多 64 MiB。

宿主使用当前会话注册表校验工具参数，并复用模式检查、编辑保护、人工/自动审批、
权限审计、插件执行链路和工具执行 Hook。只读且无需审批的工具最多四路并发；
写入及交互工具与其它调用串行。问题和计划工具使用 Agent 专用交互流程。
脚本不能递归调用 `codemode` 或发现、调用网关。

原始嵌套结果仅供脚本处理，不自动加入模型消息。脚本失败保留已收集输出，
但不提交状态增量；已经完成的工具副作用不会撤销。未等待的调用在脚本结束时取消。
状态增量绑定会话轮次，恢复或切换分支时只读取当前祖先路径。

子进程边界防止原生引擎崩溃直接终止主进程，但不提供操作系统级 syscall 沙箱。
获批工具仍具有其自身权限；进程取消也不保证撤销已经启动的外部操作。
此实现不提供 Pi 的独立 `models.*` 接口。
