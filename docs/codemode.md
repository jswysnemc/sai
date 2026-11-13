# codemode 工具模式

codemode 是工具调用方式，与 YOLO、人工审核、自动审核、计划等权限模式独立。
默认采用混合方式：模型既可直接调用普通工具，也可通过 JavaScript 编排多个工具。
普通调用和脚本共用工具说明、参数定义、已加载状态与权限检查。

- `/code-model on`：普通调用与脚本并存；单次操作优先直接调用，循环、依赖处理和中间结果过滤可使用脚本。
- `/code-model off`：只使用普通工具。
- `/code-model only`：业务工具只能通过脚本执行，保留原生 `load` 和 `request_capability` 发现入口。
- `/code-model`：切换混合与关闭状态；从独占状态切换时关闭脚本。

命令行对应 `--code-model`、`--no-code-model`、`--code-model-only`，三者互斥。
例如 `sai --yolo --code-model ask "任务内容"`，也可组合 `--audited`、`--auto-audit` 或 `--plan`。
只指定工具方式时沿用对应入口的默认权限。Web 的“运行时 → 工具与上下文 → 默认工具方式”可修改全局默认值。
Shift+Tab 按 YOLO → AUDIT → AUTO-AUDIT → YOLO 循环，并保留工具方式；计划模式需显式退出。
切换在输入期间和模型执行期间均可使用，下一次请求使用新的工具声明。
关闭脚本时不暴露入口，工具分组目录始终不将它列入业务工具分组。

此模式用 JavaScript 编排 Sai 工具。模型通过内部执行入口 `codemode` 传入
`{"code":"..."}`；脚本作为异步函数体执行，支持顶层 `await` 和 `return`。
它需要当前 Agent 的执行桥接，不能通过独立 `__tool` 命令执行。

## 配置

在 `config.jsonc` 中配置：

```json
{
  "codemode": {
    "mode": "on",
    "inline_budget": 3000,
    "timeout_ms": 60000
  }
}
```

- `mode`：`"on"` 为默认混合方式，`"only"` 为显式脚本独占，`"off"` 关闭脚本。已有的 `"on"` 不再强制独占；显式 `"off"` 保持关闭。
- 混合方式的脚本说明不内联任何业务工具描述。常驻工具使用原生声明；延迟工具复用 Jev 或 `load` 返回的定义。
- `inline_budget`：仅用于独占方式的常驻工具声明，最大 32000；延迟工具加载后的定义不再次内联。混合方式忽略此预算。
- `timeout_ms`：整个脚本的硬超时，包含启动、工具执行及审批等待，范围为 1–600000。

终端 `/code-model` 命令同时改变当前会话并保存 `codemode.mode` 全局默认值，后续启动沿用所选方式；空闲和模型运行期间均可保存，保存失败时保留原模式。命令行参数只覆盖当次启动，不改写全局默认值。权限命令和 Shift+Tab 不保存或改写工具方式默认值。Web 请求只指定权限时合并全局工具方式，入队时固定完整组合；显式 `yolo+native`、`yolo+code-model` 或 `yolo+code-model-only` 优先于默认值。
请求间隙消息只有与 Agent 当前实时模式相同时才合并；不同权限或工具方式的请求保留在队列中独立执行。

脚本入口由会话模式提供，无需加入 `enabled_tools`。嵌套工具仍受当前档案白名单约束，
脚本不能访问过滤后的注册表之外或尚未加载的工具。YOLO 与 codemode 组合时无需逐条确认；
人工审核、自动审核和计划模式分别沿用各自权限与沙箱策略，启停 codemode 不改变权限。

终端标题与状态栏同时展示权限和工具方式。嵌套审批附着在脚本卡片上，并显示实际工具、
命令及作用范围；等待期间脚本截止仍然有效。运行中和历史记录中的 JavaScript 使用语法高亮。

旧会话值 `code-model` 按“人工审核 + 混合工具”读取，避免恢复会话时扩大权限。
新组合使用 `yolo+code-model`、`audited+code-model`、`auto-audit+code-model`、
`plan+code-model` 保存和传递；独占方式使用 `+code-model-only` 后缀。
存储层原有 `yolo` 等值仍表示普通工具方式，权限切换及计划审批保留当前工具方式。

## 说明共享与按需加载

| 工具状态 | 完整说明位置 | 普通调用 | 脚本调用 |
| --- | --- | --- | --- |
| 常驻 | 原生工具声明；独占方式改放在脚本入口中 | 直接调用工具 | `tools[name](args)` |
| Jev 或 load 已加载 | 预选注入或加载结果 | `invoke_tool`，独占方式禁用 | `tools[name](args)` |
| 未加载 | 必要的发现信息 | 先发现和加载 | 先通过原生入口发现和加载 |

加载一次即可供两条执行路径使用；延迟工具不会在加载后再加入原生工具声明。
压缩清除加载状态后，需要重新取得定义。显式重读工具说明仍可通过 `load` 完成。
“一份说明”指同一有效请求上下文中不为两种调用方式重复提供定义；无状态模型接口仍可能随每次请求发送声明和历史消息。

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
- `ALL_TOOLS`：当前已暴露工具的名称和描述，不包含隐藏工具。
- `await searchTools(query, {limit, namespace})`：查询同一已暴露集合，默认返回 8 项，不触发 Jev 或加载。
- `await describeTool(name)`：返回已暴露工具的名称、说明和原始参数 Schema，不重复返回 TypeScript 声明；未暴露时返回 `undefined`。
- `store(key, value)` / `load(key)`：存取分支内 JSON 状态；保存 `undefined` 删除键，
  保存 `null` 不删除键。单值最大 256 KiB，整个状态最大 1 MiB。

脚本中的 `load(key)` 只读取状态，与原生 `load` 工具无关。需要新工具时先结束脚本，通过原生 `load` 或 `request_capability` 加载，再开始下一次脚本调用。

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
