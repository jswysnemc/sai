# 原生桌面操作

桌面工具族用于操作本机原生应用，与通过 CDP 控制网页的 `browser` 工具相互独立。工具族借鉴 Prux 的分动作入口、截图帧与无障碍操作设计，保留 Sai 的应用定向、快照句柄和后台投递约束。底层仍通过 `cua-driver` 的常驻 MCP 连接访问系统能力，不依赖 Prux 或 `cc-haha` 的运行进程及源码目录。

## 启用

1. 安装 `cua-driver` 0.34.1 或更新版本，并按照驱动说明授予系统权限。Web 设置页可在用户确认后把官方发布包下载到 Sai 数据目录；`sai` 不会自动开启功能、授予系统权限或改写 PATH。
2. 在 Sai 的 `config.jsonc` 中显式开启功能。Linux 默认路径为 `~/.config/sai/config.jsonc`，其他平台使用对应配置目录。

```jsonc
{
  "computer_use": {
    "enabled": true,
    "driver_path": "cua-driver",
    "timeout_ms": 30000
  }
}
```

`driver_path` 可以是带空格的绝对路径；Sai 直接执行该文件，不经过 Shell。`timeout_ms` 范围为 1000 至 120000，覆盖初始化、排队和本次请求。配置修改后重新进入会话，使新工具注册表生效。

该功能默认关闭，旧配置不会在升级后自动获得桌面访问能力。启用后，应用枚举、窗口观察、截图和输入仍遵守当前 Sai 会话的权限模式；计划模式不会执行这些操作。`computer_use` 工具的 `status` 动作只读取配置，不启动驱动。Web 设置页可以编辑开关、路径和超时，并在确认后安装官方二进制。

渐进加载使用 `computer_use` 工具组，统一入口和所有分动作入口都属于这一组。Agent 档案如果配置了工具白名单，需要逐个允许所用入口；只允许 `computer_use` 不会自动允许全部分动作入口。子代理不会继承桌面工具族，也不能通过工具目录重新加载它。

## 工具入口

保留原有 `computer_use` 统一入口；以下分动作入口共用同一驱动连接与快照：

| 入口 | 对应统一动作 | 用途 |
| --- | --- | --- |
| `computer_status` | `status` | 只读取配置，不访问桌面 |
| `computer_apps` | `list_apps` | 枚举运行应用 |
| `computer_windows` | `list_windows` | 列出指定应用的窗口 |
| `computer_screenshot` | `get_app_state` | 窗口截图与无障碍状态，支持裁剪、缩放 |
| `computer_ui` | `get_app_state` | 只读取无障碍状态，不请求截图 |
| `computer_click` | `click` | 点击当前元素或截图中的位置 |
| `computer_drag` | `drag` | 在同一窗口截图内拖拽 |
| `computer_scroll` | `scroll` | 滚动指定窗口或控件 |
| `computer_type` | `type_text` | 向指定窗口或控件输入文本 |
| `computer_key` | `press_key` | 发送按键或组合键 |
| `computer_ui_action` | 见下文 | 查询或操作当前无障碍控件 |
| `computer_stop` | `reset` | 关闭本会话连接并清除快照 |

分动作入口不接受 `action` 覆盖，只有 `computer_ui_action` 需要显式动作。它支持 `get_value`、`get_text`、`locate_text`、`invoke`、`show_menu` 和 `set_value`。

## 操作流程

按顺序调用同一个工具：

```json
{"action":"status"}
{"action":"list_apps"}
{"action":"list_windows","app":"1234"}
{"action":"get_app_state","app":"1234","window_id":101}
```

`status` 只读取当前配置，不启动驱动，也不代表系统权限或驱动可用性已经通过检查。其他调用首次连接时才启动 `cua-driver mcp`。

`app` 接受列表中的正数进程号，也接受能够唯一匹配的运行应用名称或 `bundle_id`。应用重名时必须使用进程号；不会自动启动应用或退回前台应用。`window_id` 必须来自目标应用的窗口列表。

`get_app_state` 返回原生可访问性状态、`snapshot_id`、形如 `g1:0` 的元素句柄及可用的窗口截图。有截图时还返回同值的 `frame_id`。截图作为图片附件交给模型并随工具历史保存，不把 base64 放进文本结果。默认保留驱动原图字节，仅显式设置裁剪或缩放时重新编码为 PNG。

相同流程也可使用 `computer_apps({})`、`computer_windows({"app":"1234"})` 和 `computer_screenshot({"app":"1234","window_id":101})`。不要混用不同应用或窗口的快照。

```json
{
  "action": "click",
  "app": "1234",
  "window_id": 101,
  "snapshot_id": "复制刚才返回的 snapshot_id",
  "element_index": "g1:0"
}
```

输入动作包括 `click`、`scroll`、`drag`、`press_key`、`type_text`、`set_value`、`invoke` 和 `show_menu`。`invoke` 使用原生控件默认动作，`show_menu` 请求控件菜单，不通过猜测屏幕位置操作。`click` 可设置 `button` 和 `click_count`；组合键使用 `key` 与 `modifiers`。`scroll` 的 `amount` 按 `by=line` 或 `by=page` 计数。

元素句柄只能取自同一快照。使用像素时，`x/y` 或拖动起终点均对应那张窗口截图，左上角为 `(0, 0)`，不是桌面全局坐标。没有截图时拒绝像素输入，但仍可使用原生元素句柄或目标窗口的键盘操作。

每次输入在发送前消费快照。输入成功、失败、超时或取消后，都必须重新调用 `get_app_state`、`computer_screenshot` 或 `computer_ui`，不能盲目重试。重新枚举应用/窗口、重置会话或同一 Sai 进程中的其他会话发送输入，也会使旧观察失效。请求并发时明确返回忙碌状态，不把旧坐标排队执行。用户手动操作或窗口移动、缩放后也应重新观察；Sai 不会自动检测所有外部窗口变化。

`reset` 关闭本会话的驱动连接并清除句柄，不停止其他客户端的驱动服务。

## 裁剪与缩放

以下参数传给 `computer_screenshot`；统一入口使用相同参数并增加 `"action":"get_app_state"`：

```json
{
  "app": "1234",
  "window_id": 101,
  "max_image_dimension": 1568,
  "region": {"x": 100, "y": 80, "width": 600, "height": 400},
  "scale": 0.5
}
```

`max_image_dimension` 控制驱动提供的原图最大尺寸，范围 64–4096，默认 1568。`region` 使用该原图中的窗口局部像素，必须完整位于原图内部。`scale` 范围 0.1–1.0，交付尺寸按实际结果取整；上例交付 300×200 图片。`computer_ui` 不接受这些图片选项；统一入口关闭截图时也不能裁剪或缩放。

模型使用交付图片中的坐标，Sai 将它投影回驱动原图。点击该图片的 `(150, 100)`，对应原图 `(400, 280)`。无需模型手动换算，也不能传桌面坐标。缺少截图或越界时拒绝像素输入。

`computer_click`、`computer_drag`、`computer_scroll`、`computer_type` 和 `computer_key` 可以用 `from_frame` 替代 `snapshot_id`：

```json
{
  "app": "1234",
  "window_id": 101,
  "from_frame": "复制刚才返回的 frame_id",
  "x": 150,
  "y": 100
}
```

两个字段同时出现时必须相同。帧只绑定当前应用、窗口和观察，不能跨快照复用。

## 快照内查询

向 `computer_ui_action` 传递动作与原观察的目标、快照：

```json
{"action":"get_text","app":"1234","window_id":101,"snapshot_id":"当前快照","element_index":"g1:0"}
```

- `get_value` 返回控件的 `value`；字段缺失时明确报错。
- `get_text` 优先返回 `value`，没有该字段时返回 `label`。
- `locate_text` 使用 `name` 不区分大小写查找 `label` 或 `value`，返回排序后的匹配句柄，不自动选择或点击。

查询读取最新授权快照，不额外访问桌面，也不消费输入资格；跨窗口、旧快照和旧句柄仍然拒绝。控件字段最长保留 256 个字符，过长字段附带 `truncated` 标志；查询不会恢复未展示的原文。匹配结果最多展示 50 项，且正文限制在约 32 KB，返回 `total_matches` 与 `truncated`。完整状态也有总量上限，需要更精确信息时应重新观察并缩小 `query` 范围。

内联与全屏视图使用相同的语义化动作标题、目标与控件详情，不直接显示协议 JSON。全屏视图将详情放入既有执行链折叠窗口。截图附件供模型和历史使用，本次没有新增终端 Kitty 内嵌截图预览。

## 平台边界

- macOS：由驱动处理辅助功能、屏幕录制权限及原生窗口操作。
- Windows：由驱动处理窗口身份、可访问性和输入投递；系统权限边界仍然有效。
- Linux：由驱动处理 X11 或 Wayland 能力。Wayland 合成器如果不能证明截图属于指定窗口，工具保留可用的结构状态并拒绝像素操作。

Sai 固定使用后台投递，不自动切换到前台输入，不抢占前台应用来掩盖能力缺失。具体可用性以驱动返回结果为准。本接入没有照搬 Prux 的全局输入后端，也不提供应用启动、全局剪贴板写入、持久 JavaScript 执行器或无观察批处理接口。

## 回归验证

```sh
cargo test --locked --bin sai computer_use
```

测试覆盖分动作入口与统一入口共享会话、渐进工具发现、禁用及计划模式、子代理隔离、截图裁剪与坐标投影、无障碍查询、快照消费和协议失败。测试使用合成图片、内存驱动、回环 MCP 服务和隔离 stdio 子进程，不调用真实桌面或模型服务。Unix 下的 stdio 子进程测试使用 Python 3 标准库。
