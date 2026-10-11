# 原生桌面操作

`computer_use` 用于操作本机原生应用，与通过 CDP 控制网页的 `browser` 工具相互独立。该工具参考应用定向、快照句柄和窗口截图坐标契约，通过 `cua-driver` 的常驻 MCP 连接访问系统能力，不依赖 `cc-haha` 的运行进程或源码目录。

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

渐进加载使用 `computer_use` 工具组。Agent 档案如果配置了工具白名单，还需要允许 `computer_use`。

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

`get_app_state` 返回原生可访问性状态、`snapshot_id`、形如 `g1:0` 的元素句柄及可用的窗口截图。截图作为图片附件交给模型，不把 base64 放进文本结果，也不二次缩放图片。

```json
{
  "action": "click",
  "app": "1234",
  "window_id": 101,
  "snapshot_id": "复制刚才返回的 snapshot_id",
  "element_index": "g1:0"
}
```

输入动作包括 `click`、`scroll`、`drag`、`press_key`、`type_text` 和 `set_value`。`click` 可设置 `button` 和 `click_count`；组合键使用 `key` 与 `modifiers`。`scroll` 的 `amount` 按 `by=line` 或 `by=page` 计数。

元素句柄只能取自同一快照。使用像素时，`x/y` 或拖动起终点均对应那张窗口截图，左上角为 `(0, 0)`，不是桌面全局坐标。没有截图时拒绝像素输入，但仍可使用原生元素句柄或目标窗口的键盘操作。

每次输入在发送前消费快照。输入成功、失败、超时或取消后，都必须重新调用 `get_app_state`，不能盲目重试。重新枚举应用/窗口、重置会话或同一 Sai 进程中的其他会话发送输入，也会使旧观察失效。请求并发时明确返回忙碌状态，不把旧坐标排队执行。用户手动操作窗口后也应重新观察。

`reset` 关闭本会话的驱动连接并清除句柄，不停止其他客户端的驱动服务。

## 平台边界

- macOS：由驱动处理辅助功能、屏幕录制权限及原生窗口操作。
- Windows：由驱动处理窗口身份、可访问性和输入投递；系统权限边界仍然有效。
- Linux：由驱动处理 X11 或 Wayland 能力。Wayland 合成器如果不能证明截图属于指定窗口，工具保留可用的结构状态并拒绝像素操作。

Sai 固定使用后台投递，不自动切换到前台输入，不抢占前台应用来掩盖能力缺失。具体可用性以驱动返回结果为准。本接入提供原生动作工具，不包含参考项目的持久 JavaScript 执行器或批处理接口。

## 回归验证

```sh
cargo test --locked --bin sai computer_use
```

测试使用合成图片、内存驱动、回环 MCP 服务和隔离 stdio 子进程，不调用真实桌面或模型服务。Unix 下的 stdio 子进程测试使用 Python 3 标准库。
