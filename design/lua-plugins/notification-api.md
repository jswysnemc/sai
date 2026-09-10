# 主动通知投递

`sai.notify.send(request)` 在当前工具或命令调用中发送桌面通知、播放声音，并等待指定通道完成。通知发生在运行 Sai 的宿主机器上，不会自动转发到浏览器或远端客户端；接口不提供后台调度、持久任务或脱离调用的工作进程。

## 声明、授权与调用权限

```json
{
  "capabilities": {
    "system": {
      "notify": true,
      "read_paths": ["audio"]
    }
  }
}
```

`system.notify` 独立控制主动投递。使用本地音频时，还需要声明并授权对应的 `system.read_paths`；桌面通知和内置声音无需文件读取授权。

```sh
sai plugins enable my-plugin --allow-notify
sai plugins enable my-plugin --allow-read-path audio
sai plugins enable my-plugin --no-notify
sai plugins info my-plugin --json
```

分项更新保留其他有效授权。`--allow-notify` 与 `--no-notify` 互斥，均不能与 `--grant-declared` 混用；未声明的能力不能获得授权。

顶层 `notifications` 及 `--allow-notifications` 只用于 `reply_end` 的纯展示策略，不授予主动投递权限。两项能力可以分别授予、撤销；纯展示运行时始终不能调用 `sai.notify.send`。

投递同时要求有效的 `system.notify` 和宿主可信写入权限。工具应声明 `access="writes"`，或在 `optional_writes` 工具取得写入许可后调用；命令必须声明 `access="writes"`。只读工具、只读命令、所有事件及初始化阶段不能投递，修改 `ctx.allow_writes` 或保存旧上下文也不能扩大权限。

## 请求与返回值

下面的函数可注册为 `access="writes"` 工具的回调：

```lua
--- 【通知示例】【即时提醒】发送文本并等待内置提示音播放完成
--- @param args table 包含 title 和可选 body
--- @return table 已完成的桌面与声音通道
local function send_notice(args)
    return sai.notify.send({
        title = args.title,
        body = args.body or "",
        desktop = true,
        sound = {builtin = "chime"},
        timeout_ms = 10000,
    })
end
```

| 字段 | 类型与默认值 | 约束 |
| --- | --- | --- |
| `title` | 必填字符串 | 不能只有空白，最多 256 个 UTF-8 字节 |
| `body` | 字符串，默认空串 | 最多 4096 个 UTF-8 字节 |
| `desktop` | 布尔值，默认 `true` | 是否请求桌面通知 |
| `sound` | 可选对象，默认无声音 | 使用一个内置名称或本地路径 |
| `timeout_ms` | 整数，默认 10000 | 1–120000 毫秒，实际时限不超过回调限制 |

请求和声音对象拒绝未知字段及错误类型；标题和正文拒绝换行、回车、制表符之外的控制字符。宿主不会自动截断文本。必须至少请求一个通道，`desktop=false` 且省略声音会报错。

声音只能选择以下一种形式：

```lua
sound = {builtin = "alarm"}
sound = {builtin = "chime"}
sound = {path = "audio/custom.wav"}
```

`builtin` 与 `path` 不能同时出现。内置名称只接受 `alarm` 和 `chime`，不能借名称选择其他宿主资源；`sound` 不接受纯展示通知使用的布尔值。

全部请求通道完成后返回 `{desktop=true, sound=true}` 等对象，未请求的通道为 `false`。桌面成功表示系统通知程序成功退出，声音成功表示宿主完成播放；这不证明用户实际看见或听见通知。任一通道失败都会产生 Lua 错误，宿主不会把失败通道报告为成功，也不会隐式改用其他通道。插件可以用 `pcall` 捕获错误并决定后续行为。

## 音频文件边界

本地音频复用[系统接口](system-api.md)的路径授权。相对路径以 Rust 保存的本次工作目录为基准，不以插件安装目录为基准；Lua 修改 `ctx.workdir` 无效。绝对路径和用户目录语法遵循同一授权规则。

文件必须是非空普通文件，最多 8 MiB。宿主先验证真实路径是否属于读取范围，再使用授权目录句柄打开并读取有界快照；拒绝目录、设备、管道和越界链接。解析后仍在授权范围内的符号链接沿用普通文件接口语义，授权后的链接替换不能扩大读取范围。

解码器按内容识别 WAV 或 MP3，不依赖文件扩展名；内容损坏或不支持的格式会报错。文件读取不返回原始字节给 Lua，也不开放任意音频设备、命令或网络来源选择。

## 时限、取消与部分完成

文件读取、桌面程序和声音播放共用一次请求时限。每次实际进入宿主的请求消耗一个 `limits.system_calls` 额度，两个通道仍只计一次；宿主错误和超时也计数。参数或运行时授权校验失败不计数。额度与文件、环境、进程及私有存储接口共享，每次回调重新计算。

单次超时或外部取消会释放等待中的宿主 Future，并通知原生工作线程停止后续动作。桌面等待与音频播放每隔约 20 毫秒检查取消：桌面子进程会终止并回收，声音播放会停止并释放相关资源。回调总截止时间不会因新的通知请求而延长，Lua 修改 `sai.limits` 无效。

取消不能强制中断已经开始的原生文件系统操作、音频解码或设备初始化。控制状态在这些调用前后再次检查；若底层调用尚未返回，资源回收可能晚于 Lua 超时。

组合投递先读取本地音频，再发送桌面通知，最后解码并播放声音。已经显示的通知和已经播放的声音无法回滚；例如音频解码失败时，桌面通知可能已完成。接口不提供全有或全无、重试去重或恰好一次交付保证。

## 平台适配与验证范围

Linux 使用固定的 `notify-send`，macOS 使用固定的 `osascript` 脚本，标题和正文仅作为独立参数传递，不拼接进 shell 命令或脚本。缺少程序、桌面服务不可用或程序失败会返回错误。Windows 及其他没有桌面适配的平台明确报告不可用；可以显式设置 `desktop=false` 请求声音通道。

声音使用宿主默认音频输出，设备不可用时返回错误。本接口不读取旧答复通知的开关或声音设置，插件负责自己的业务策略。

第十五轮验证包含运行时权限与取消契约、宿主文件边界、隔离桌面记录程序，以及通过 ALSA 空输出执行的内置声音、WAV 和 MP3 播放。验证没有向真实桌面发送通知，也没有使用物理扬声器；macOS 与 Windows 的平台运行效果没有在本轮 Linux 环境中验证。

[Lua 闹钟](alarm.md)通过独立授权的[持久命令调度](scheduler-api.md)调用 `deliver`，默认只播放一次内置声音。旧记录与旧入口由受限兼容层接入，投递失败保存为明确任务错误，详见[迁移记录](migration.md)。
