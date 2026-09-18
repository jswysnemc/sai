# TUI 底栏纯回调

插件声明 `"capabilities": {"tui_status": true}`，用户启用时通过 `--allow-tui-status` 授予这一项。`--no-tui-status` 撤销授权；通知、文件、模型等授权与此独立。仅启用插件不自动授权。

```lua
--- 【底栏示例】【状态展示】返回左右状态文本
--- @param event table 宿主已有的界面状态
--- @return table 左右单行文本；返回 nil 时继续使用其他插件或默认底栏
local function render(event)
    return {left = event.mode .. " · " .. event.model, right = event.directory}
end
sai.on("tui_status", render)
```

## 状态与结果

| 输入字段 | 类型 | 含义 |
| --- | --- | --- |
| `columns` | 整数 | 当前终端总列数，宿主另保留底栏两侧间距 |
| `locale` | 字符串 | `zh-CN` 或 `en-US` |
| `mode` | 字符串 | `yolo`、`audit`、`auto-audit` 或 `plan` |
| `model`、`thinking` | 字符串 | 当前模型名称与思考等级 |
| `directory` | 字符串 | 当前目录的界面文本；家目录前缀压缩为 `~` |
| `context_ratio` | 数字 | 上下文占用比例；1 表示 100% |
| `context_window_tokens` | 整数 | 上下文窗口大小 |
| `cache_hit_ratio` | 数字或 null | 当前轮累计缓存命中率；Lua 中须先检查 `type(...) == "number"` |

返回 `nil` 或 `{left=string, right=string}`，不接受额外字段。每侧最多 2048 字节，禁止控制字符、换行和 Unicode 行分隔符。一个包的多个监听器至多返回一个非空布局，否则整包失败。宿主统一着色、按字符显示宽度裁剪；工作状态和停止快捷键由宿主保留。

输入总量最多 16 KiB。不会交付用户输入、对话正文、凭据或会话存储。`ctx.session_id`、`ctx.workdir` 为空，`ctx.allow_writes` 为 false。

## 隔离、刷新与回退

展示使用独立 VM，只有底栏能力。即使同一包获得其他授权，此 VM 也不能访问网络、文件、进程、模型、工具、终端或存储。Lua 堆最多 4 MiB、指令最多 100,000，加载和每次回调各限 100 毫秒，序列化输出最多 16 KiB。更小的清单预算继续有效。

每次 TUI 进入新的输入循环时读取已保存的源码、设置、启用状态和授权；该循环内复用独立实例。状态变化时在后台计算，连续请求合并成最新快照；相同快照不重复执行 Lua。渲染线程只读缓存；首次结果未就绪或当前快照尚未完成时使用默认底栏。输入循环最多每 100 毫秒检查一次结果，其他绘制事件可更早更新。

最多加载八个已启用且授权的底栏包，按插件 ID 排序。第一个非空合法布局生效；返回 nil 时尝试后续包。加载或回调错误通过 TUI 普通消息显示，本轮不再执行该失败包；其他插件仍可提供布局，没有有效结果时使用默认底栏。配置和撤权在下一次输入循环生效。

本接口只定制 TUI 底栏，不改变模型请求、工具定义、Web 页面或会话运行状态。参考实现为 [tui-status](../../examples/lua-plugins/tui-status/README.md)。
