# 文件、环境与进程接口

Lua 业务通过 `sai.fs`、`sai.env` 和 `sai.process` 访问系统能力。运行时校验输入、有效授权、可信调用权限和预算；Sai 宿主负责真实路径、平台进程与资源回收。`io`、`os` 和原生动态库仍未开放。

## 能力声明与授权

```json
{
  "capabilities": {
    "system": {
      "read_paths": [".", "/etc/os-release"],
      "environment": ["LANG", "PATH"],
      "processes": {
        "inspect-process": {
          "program": "pgrep",
          "args": ["-af", "--", {"parameter": "target"}],
          "parameters": {
            "type": "object",
            "properties": {"target": {"type": "string", "minLength": 1, "maxLength": 160}},
            "required": ["target"],
            "additionalProperties": false
          },
          "read_only": true
        }
      }
    }
  },
  "limits": {"system_calls": 1024}
}
```

路径、环境名称和进程模板各自最多 64 项。路径是精确声明的文件或目录，支持绝对路径、相对于可信工作目录的路径以及 `~`、`~/…`。禁止父目录跳转、通配符和控制字符。环境名称只允许 ASCII 字母、数字和下划线，不能以数字开头。

外部包缺省没有系统授权。下列命令分别选择清单已声明的能力：

```sh
sai plugins enable my-plugin --allow-read-path . --allow-env LANG --allow-process inspect-process
sai plugins enable my-plugin --no-file-read
sai plugins enable my-plugin --no-env
sai plugins enable my-plugin --no-processes
sai plugins info my-plugin --json
```

同一类可重复使用 `--allow-*` 指定多个值，该类授权会替换为所选集合；未指定类别保持有效授权。`--grant-declared` 授予整个当前清单，不能与分项选项混用。路径和环境按精确值求交集；进程模板按名称及全部字段求交集，修改程序、参数、Schema、`read_only` 或 `workspace` 都不能复用旧的显式授权。过期授权不会在修改其他类别时恢复；重新授予模板才认可新内容。

系统授权与 `tools` 授权相互独立。撤销 `system.processes` 不会隐式撤销已授权的 `run_command`；撤销文件读取也不会撤销 `read_file` 工具。

## 平台与环境

```lua
local platform = sai.system.platform
local host_pid = sai.system.process_id
local language = sai.env.get("LANG")
```

`platform` 是宿主实际平台名，例如 `linux`、`macos` 或 `windows`；`process_id` 是当前 Sai 进程号。两个元数据字段在加载期即可读取。

`sai.env.get(name)` 只在回调内开放，返回 UTF-8 字符串或 `nil`。未授权名称、无效编码或超过输出预算的值返回错误。授权文件路径中的 `~` 由宿主解析，不会同时授予读取 `HOME` 的权限。

## 文件与目录

```lua
local file = sai.fs.read_text("notes.txt", {max_bytes=65536, lossy=false})
local directory = sai.fs.read_dir(".", {max_entries=100})
local information = sai.fs.stat("notes.txt")
```

| 接口 | 结果 | 默认限制 |
| --- | --- | --- |
| `read_text(path, options)` | `{text, truncated}` | 1 MiB，进一步受回调输出上限限制 |
| `read_dir(path, options)` | `{entries, truncated}` | 256 条，最多 1024 条 |
| `stat(path)` | `{is_file, is_dir, len}` 或 `nil` | 不读取正文 |

目录条目包含 `name`、`path`、`is_dir`、`is_file`。`len` 是文件系统报告的字节数；例如 `/proc` 伪文件可能报告零长度但仍有可读正文。`truncated=true` 表示还有内容未返回，不能据此认定扫描完整。

请求的字节数和条数收窄到至少 1。`lossy` 默认为 `false`：严格读取遇到非法 UTF-8 会失败；字节上限切到一个有效字符中间时丢弃不完整尾部。`lossy=true` 使用替换字符，替换后的文本仍不能突破字节限制。运行时在接收宿主结果后再次检查单次请求和总结果大小。

读取仅接受普通文件，拒绝管道和设备等特殊文件。宿主解析真实路径和最近存在的祖先，检查其是否属于授权范围，再逐级打开不跟随替换链接的目录句柄。单文件授权不能读取相邻文件或枚举父目录；末级链接替换也不能借父目录句柄扩大授权。目录条目不提供越过已打开目录边界的链接目标属性。

`stat` 对已授权但不存在的路径返回 `nil`；权限不足或越界是错误。相对路径始终使用 Rust 保存的本次工作目录，修改 Lua 的 `ctx.workdir` 不会改变文件和模板进程接口的工作目录。

文件操作使用阻塞工作线程。取消会停止当前 Lua 调用并丢弃结果，但不能承诺强制中断已经开始的阻塞文件系统操作。

## 模板进程

```lua
local result = sai.process.output("inspect-process", {target="example"}, {
    timeout_ms=2000,
    max_stdout_bytes=32768,
    max_stderr_bytes=8192,
})
```

返回字段为 `status`、`stdout`、`stderr`、`timed_out`、`stdout_truncated`、`stderr_truncated`。正常退出时 `status` 是退出码；无法取得退出码时为 JSON null。执行失败是 Lua 可捕获的错误。单次超时返回 `timed_out=true`、null 状态和空输出，不保留超时前的部分管道内容。

默认时限为 30 秒，收窄到回调总时长以内且不超过 120 秒。stdout 默认 64 KiB，stderr 默认 16 KiB；每路最多占回调输出预算的一半。宿主同时排空两路管道，只保留限定字节，并在截断时维持合法 UTF-8。运行时继续检查两路各自的请求限制和序列化后的总大小。

模板程序固定，参数最多 64 项。字面量直接进入 argv；`{"parameter":"name"}` 只能引用 Schema 声明的字段，值必须是已通过校验的字符串、数字或布尔值，并占据完整的一个 argv 位置。程序和单个参数最多 8192 字节，不接受 NUL；参数总量和参数 JSON 各限 64 KiB。Schema 必须是对象、拒绝额外字段，最多 64 KiB 且只允许本地引用。省略 `parameters` 时只接受空对象。

程序名称使用宿主 PATH 中的绝对目录定位，跳过非可执行文件；明确的相对程序路径以可信工作目录为基准。PATH 用于程序定位，不代表它会传给子进程。子进程清空继承环境，只加入 `system.environment` 明确授权的变量。没有 shell 字符串拼接；Windows 的 `.cmd`、`.bat` 必须通过显式解释器模板执行。

`read_only` 默认为 `false`。写入模板必须同时满足：插件工具或命令声明 `access="writes"`，且当前宿主调用允许写入。只读工具和所有事件都不能借修改 Lua 上下文获得写入权限。

`read_only=true` 是用户授予的完整模板契约，不是操作系统沙箱，也不证明任意程序没有副作用。模板中的程序、选项和变量范围必须体现声明的用途；将解释器或任意目标执行模板标记为只读会扩大信任范围。

Unix 进程使用独立进程组；结束和取消时终止所属进程组，再回收组长 PID，避免 PID 重用误伤其他调用。新建会话等主动脱离进程组的行为不属于该组的回收范围。Windows 在挂起状态创建进程，加入关闭即终止的 Job 后恢复初始线程；结束或取消时关闭 Job。模板进程不会自动提升为 Sai 后台任务，两路输出读取也不会脱离所属调用。

## 调用作用域与预算

环境、文件和模板进程共用 `limits.system_calls`：默认每次回调 1024 次，上限 4096 次。输入或能力校验失败不执行宿主操作；实际进入宿主后发生错误或超时仍计数。每次工具、命令或事件开始时重置预算，完成、失败和取消后撤销本次系统上下文。

初始化阶段只能读取平台元数据，不能进行环境、文件和进程 I/O。事件可以使用已授权的只读系统能力，但没有模型、工具调用或写入权限。单次进程时限不能延长回调总截止时间。

私有会话状态、缓存目录、tar.gz 展开及工作目录进程见[私有接口](private-api.md)。普通进程模板默认 `workspace=false`，不能通过私有目录入口改变其工作目录类型。
