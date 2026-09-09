# 私有状态与工作目录

`sai.storage` 保存插件自己的会话记录；`sai.workspace` 管理插件私有缓存目录。两项能力独立声明和授权，不提供任意路径写入接口。普通文件、环境和进程接口见[系统接口](system-api.md)。

## 授权

```json
{
  "capabilities": {
    "http": ["https://aur.archlinux.org"],
    "system": {
      "session_storage": true,
      "workspace": true,
      "processes": {
        "build": {
          "program": "makepkg",
          "args": ["--noconfirm"],
          "workspace": true
        }
      }
    }
  }
}
```

```sh
sai plugins enable my-plugin --allow-session-storage --allow-workspace
sai plugins enable my-plugin --no-session-storage
sai plugins enable my-plugin --no-workspace
```

未指定的类别保持原授权。外部包缺省没有上述能力；`--grant-declared` 可以一次授予清单的完整声明。`workspace` 是进程模板的一部分，更改它会使旧的显式模板授权失效。普通 `sai.process.output` 不能调用私有目录模板，私有目录也不能使用普通目录模板。

## 会话记录

```lua
local old = sai.storage.get("review/example")
sai.storage.set("review/example", {accepted=true})
local changed = sai.storage.compare_exchange("review/example", {accepted=true}, {accepted=false})
sai.storage.set("review/example", nil)
```

| 操作 | 结果 |
| --- | --- |
| `get(key)` | 已保存的 JSON 值，缺失返回 null |
| `set(key, value)` | 已保存值；nil 或 JSON null 表示删除 |
| `compare_exchange(key, expected, value)` | 当前值与 expected 相等时原子替换并返回 true，否则返回 false |

Lua 可用 `value == nil or value == sai.json.null` 判断缺失值。比较按 JSON 值进行，不比较对象字段顺序。键是 1–256 字节且不含控制字符的文本；宿主将键转换成摘要文件名。每个插件、每个会话最多 128 个键，单个值的 JSON 编码不超过 256 KiB。写入使用同目录临时文件、同步和原子替换。跨进程的短文件锁覆盖比较与写入；发生并发冲突时返回错误，由调用方决定是否重试。

目录归属由 Rust 绑定的插件标识和完整会话作用域决定。正式 Agent 使用会话目录区分工作区内同名会话；子 Agent 独立隔离。插件 A 调用 B 时，B 使用自身插件命名空间及原用户会话的状态；新建 Lua VM 不会改变这一归属。直接 CLI 工具调用使用单独的 `direct-command` 作用域，允许两个独立命令进程先审查、后确认。

只读工具可以更新已授权的私有记录，也可以创建私有缓存。事件只能读取记录，不能调用 set、compare_exchange 或创建工作目录。修改 Lua 上下文字段不会改变宿主归属。任何入口重置 `StateStore` 都会清除当前会话的插件记录，其他会话不受影响；清空会话命令也清理直接 CLI 工具记录。

## 工作目录

```lua
local work = sai.workspace.open("review/example")
work:extract_tar_gz({
    url="https://aur.archlinux.org/cgit/aur.git/snapshot/example.tar.gz",
    destination="snapshot",
})
local listing = work:read_dir("snapshot", 256)
local file = work:read_text("snapshot/example/PKGBUILD", {max_bytes=96004})
local info = work:stat("snapshot/example/PKGBUILD")
local display_path = work:path()
```

`open(key)` 重建同一个插件、会话和键对应的缓存目录。每个回调最多打开 4 个目录，每个插件会话最多保留 128 个目录键。相同目录正被占用时返回错误；不会删除另一个调用正在使用的目录。目录锁在回调结束、失败或取消时释放，保存到 Lua 全局变量的旧句柄会失效。

缓存位于应用缓存根目录中的 `plugin-workspaces`，内容在回调结束后保留供检查，再次 open 同键时替换。会话重置撤销状态记录，不删除供检查的缓存。`path()` 只提供显示路径，不增加普通文件接口的授权。

其余路径都是规范相对路径，根目录使用 `.`。拒绝父目录跳转、绝对路径、反斜杠、盘符、控制字符和 Windows 设备名。读取遵守原文件接口的链接与普通文件限制。`read_text` 默认 64 KiB，最多占回调输出预算的一半；`read_dir` 默认 256 条，最多 1024 条，并返回相对路径。两项结果均带截断标记。

## 归档展开

`extract_tar_gz` 只接受 GET 下载。初始来源及每次重定向继续使用普通 HTTP 授权，最多 5 次重定向；下载字节不经过文本解码。

| 选项 | 默认值 | 硬上限 |
| --- | --- | --- |
| `max_bytes` | 8 MiB | 8 MiB |
| `max_unpacked_bytes` | 32 MiB | 64 MiB |
| `max_entries` | 1024 | 4096 |
| `timeout_ms` | 30 秒 | 120 秒，且不超过回调时限 |

`destination` 必须是尚不存在的子目录。只展开普通文件与目录，拒绝软链接、硬链接、特殊设备、重复路径和越界路径。Git 快照中的 PAX 全局元数据只读取跳过，累计最多 128 KiB，并计入条目数，不生成文件。普通文件保留执行位，去除特殊权限位。解压始终写入暂存目录，完整成功后才重命名发布。展开字节、条目和归档元数据同时受限；失败不发布部分目录。

取消会通知阻塞解压线程在条目或文件块边界停止；线程持有目录锁直到清理完成。已启动的阻塞 I/O 无法强制中断，但取消之后不会发布归档目录。

## 私有目录进程

```lua
local result = work:process("build", {}, {
    directory="snapshot/example",
    timeout_ms=1800000,
    max_stdout_bytes=65536,
    max_stderr_bytes=16384,
})
```

只有声明 `workspace=true` 的完整模板可以使用这一入口。目录必须位于当前句柄内，程序、参数和环境继续采用原进程能力规则。`read_only` 默认为 false，写入模板仍要求工具或命令声明 writes 并通过宿主权限检查。模板契约不是操作系统沙箱；私有目录限制的是 API 路径及工作目录选择，不能限制已授权程序自身的系统调用。

默认超时 30 秒，私有目录进程最多 1800 秒，并受整个回调时限约束。普通目录进程保持 120 秒上限。两种入口使用同一进程树回收实现和有界输出规则。为覆盖构建与安装多个步骤，清单的回调总时长硬上限为 3600 秒，缺省仍为 20 秒。

## AUR 插件的业务约束

`package-advisor` 保留 `review_aur_package` 和 `install_aur_package`。前者只读，后者声明 writes；旧 `plugins.package_advisor.enabled` 只决定缺省启用状态，独立插件配置优先。

审查优先使用可用的 paru，其次 yay，两者都不可用时才下载官方快照；选中的助手执行失败即停止，不再尝试其他助手。审查不执行 PKGBUILD。风险模式、深度两层、80 个文件和每文件 24000 个 Unicode 字符沿用原设计。截断或无法读取的证据会使 `review_complete=false`，禁止安装。新审查开始即撤销旧许可，报告超过输出预算时不留下可安装记录。旧版全局 `aur-review-state.json` 不会迁入新的会话状态，需要重新审查。

安装要求 `user_confirmed=true`、Linux 平台、完整且允许安装的审查，以及不同的 `ctx.operation_id`。同一次 Agent 请求及其插件组合调用共享操作标识，因此创建新 Lua VM 不能绕过分轮检查。布尔参数仍由调用方依据用户回复填写；它不解析或证明用户回复内容，实际写入继续依赖宿主权限流程。

审查记录通过比较交换支持一次安装尝试。助手安装失败、构建失败或取消后再次安装需要重新审查；同一记录不能重复启动安装。安装同样选择首个可用的 paru 或 yay，选中后失败即停止；两者都不可用时才采用快照、makepkg 和 pacman -U，排除签名文件作为安装产物。安装阶段仍可能重新下载可变的远端内容，当前记录不是对安装字节的签名或固定快照承诺。
