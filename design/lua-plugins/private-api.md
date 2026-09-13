# 私有状态与工作目录

`sai.storage.get/set/compare_exchange` 保存插件自己的会话记录；`sai.storage.plugin` 保存同一插件的跨会话记录，并提供私有作用域锁；`sai.workspace` 管理插件私有缓存目录。三项能力独立声明和授权，不提供任意路径写入接口。普通文件、环境和进程接口见[系统接口](system-api.md)。

## 授权

```json
{
  "capabilities": {
    "http": ["https://aur.archlinux.org"],
    "system": {
      "session_storage": true,
      "plugin_storage": true,
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
sai plugins enable my-plugin --allow-plugin-storage
sai plugins enable my-plugin --no-session-storage
sai plugins enable my-plugin --no-plugin-storage
sai plugins enable my-plugin --no-workspace
```

未指定的类别保持原授权。外部包缺省没有上述能力；`--grant-declared` 可以一次授予清单的完整声明。`workspace` 是进程模板的一部分，更改它会使旧的显式模板授权失效。普通 `sai.process.output` 不能调用私有目录模板，私有目录也不能使用普通目录模板。

`system.plugin_storage` 不继承 `session_storage` 授权，旧清单和旧授权默认没有跨会话存储。`--allow-plugin-storage` 与 `--no-plugin-storage` 互斥，两者也不能与 `--grant-declared` 同时使用。未声明该能力的插件无法获得授权；单独修改存储授权会保留其他有效授权。

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

Lua 可用 `value == nil or value == sai.json.null` 判断缺失值。比较按 JSON 值进行，不比较对象字段顺序。键是 1–256 字节且不含控制字符的文本；普通存储将键转换成摘要文件名。每个插件、每个会话最多 128 个键，单个值的 JSON 编码不超过 256 KiB。写入使用同目录临时文件、同步和原子替换。跨进程的短文件锁覆盖读取、比较与写入；锁忙立即返回错误，由调用方决定是否重试，比较不匹配则返回 false。

目录归属由 Rust 绑定的插件标识和完整会话作用域决定。正式 Agent 使用会话目录区分工作区内同名会话；子 Agent 独立隔离。插件 A 调用 B 时，B 使用自身插件命名空间及原用户会话的状态；新建 Lua VM 不会改变这一归属。`plugins call/run --session <ID>` 使用当前工作区对应的真实会话目录，省略时共用按规范工作目录隔离的 `plugin-command/<目录>` 作用域，允许跨进程先审查、后确认。原生直接工具入口使用 `cli-tool`，未绑定会话的工具注册表使用 `direct-command`。

只读工具和命令可以更新已授权的会话记录，也可以创建私有缓存。事件和 `after_tool` 策略只能读取记录，不能调用 set、compare_exchange 或创建工作目录；后者也没有模型和工具调用服务。修改 Lua 上下文字段不会改变宿主归属。`StateStore` 重置会清除当前作用域的普通插件会话记录，其他会话不受影响；清空会话命令也清理直接 CLI 工具记录。整体清空或删除会话按完整会话目录清理全部插件记录；其他工作区的同名会话和跨会话持久记录保留。公共会话记录位于应用状态目录的 `plugin-state/`，不计入会话目录字节统计。

### 待办示例与旧记录导入

普通 `todo` 示例在自己的公共会话存储中使用键 `plan`，保存 `{version:1, items, history}`。宿主不按插件 ID 读取 `todos.plugin.json`、`todos.json` 或 `todos.history.json`，同名外部包没有特殊身份。旧数据由用户明确传入完整状态，或把快照放入已声明且已授权的目录，再调用 `import` 命令；支持版本 0 和 1，拒绝覆盖非空计划或历史。

禁用、撤权和卸载保留公共计划。`reset_conversation()` 和 `sai clear --yes` 按公共规则清除当前计划与历史，保留会话目录内的旧快照；整体清空或删除会话还会删除该目录内的旧文件。导入来源不会因导入而改写或删除，需长期保留的备份应放在会话目录外。

待办使用公共存储锁、256 KiB 上限、普通文件和无链接目录约束。活动项与历史一次比较交换提交，Lua 最多重试 16 次比较不匹配；锁忙和文件错误直接失败。只读 `snapshot` 允许初始化空记录和完成归档，不导入旧文件；观察提醒回调只读取。文件发布成功后不能随外层取消回滚。完整业务与查询边界见[会话待办插件](todo.md)。

## 插件持久记录

以下片段在已授权的写入回调中执行：

```lua
local store = sai.storage.plugin
local record = store.get("preferences")
store.set("preferences", {enabled=true})
local changed = store.compare_exchange("preferences", {enabled=true}, {enabled=false})
store.set("preferences", nil)
```

三个操作的 JSON 返回值、null 删除和比较语义与会话记录相同。键必须是 1–256 字节且不含控制字符的 UTF-8 字符串；新接口拒绝数字等类型的隐式转换。每个插件最多 128 个键，单条记录以及比较值的 JSON 编码分别不超过 256 KiB。存储结果还受 `limits.output_bytes` 限制。输入与权限校验通过后，每次操作消耗一次 `limits.system_calls`，与会话存储及其他系统接口共用预算；宿主失败也会计费，每次回调重新获得额度。

| 调用场景 | 读取 | 写入与比较交换 |
| --- | --- | --- |
| 初始化或回调失效后 | 拒绝 | 拒绝 |
| 已授权的只读工具、只读命令和事件 | 允许 | 拒绝 |
| 已授权的写入工具或命令 | 允许 | 还需可信 `allow_writes=true` |
| 已授权的 `optional_writes` 工具 | 允许 | 还需可信 `allow_writes=true` |

删除与比较不匹配的 `compare_exchange` 也按写入操作检查权限。Rust 运行时和文件宿主分别验证能力与写入权限；修改 `ctx.allow_writes`、`ctx.session_id` 或 `sai.plugin_id` 不会改变授权和归属。宿主实现通过 `PluginHost::plugin_storage(request, capabilities, allow_writes)` 接入，旧宿主的默认实现返回不可用错误。

记录位于应用状态根目录的 `plugin-storage/<插件摘要>/<空作用域摘要>/`，由绑定的插件 ID 独占。会话记录继续位于 `plugin-state`；即使会话标识为空，两种类别也不会重叠。新建 Agent、切换工作区或会话、创建新 VM 和进程重启后，同一应用状态根目录下的同一插件仍可读取这些记录，其他插件不能读取。会话重置和 `sai clear --yes` 保留插件持久记录；禁用、撤权和移除源码也不会删除数据，重新授予同一插件 ID 后可以继续访问。需要清除记录时，已授权写入回调可以使用 `set(key, nil)`。

插件存储使用独立的 `.plugin-storage.lock`，会话清理继续使用 `.plugin-state.lock`。比较、读取和原子替换处于同一临界区，锁正被占用时立即返回可重试错误；比较不匹配返回 false。命名空间目录、记录和锁拒绝符号链接，记录和锁还要求普通文件。损坏 JSON 或过大记录会明确报错，不能被静默覆盖。

运行时在宿主调用前后检查超时和取消，失效回调不能继续发起存储操作。已开始的同步文件事务不能强制中断或回滚；调用超时、取消或结果超限不保证此前写入没有提交，调用方可在后续有效回调中读取记录确认。

## 插件作用域锁

`sai.storage.plugin.with_lock(key, callback, options?)` 复用 `system.plugin_storage`，协调同一插件的实例与进程。它不使用存储操作的短锁；回调中仍可调用 get、set 和 compare_exchange，写入许可继续独立检查。只读工具、命令和事件可以协调读取，初始化阶段拒绝取锁。

默认等待 10 秒，最多 600 秒；键最多 256 字节，每插件最多 128 个稳定锁文件。最多嵌套四层且键严格递增，回调结束、失败和取消自动释放，不向 Lua 交付句柄。锁归属、文件边界和取消限制见[私有作用域锁](private-lock-api.md)。

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

`extract_tar_gz` 只接受 GET 下载。初始来源及每次重定向继续使用普通 HTTP 授权，可以使用精确 `http` 来源或独立的 `http_read_any`；工作目录仍需 `system.workspace`。最多 5 次重定向，下载字节不经过文本解码；它不接受文本/二进制请求的 `max_redirects` 和 `read_error_body` 选项。

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

只有声明 `workspace=true` 的完整模板可以使用这一入口。目录必须位于句柄创建时绑定的真实根目录内，后续替换路径不会重新授予根目录。程序、参数和环境继续采用原进程能力规则。`read_only` 默认为 false，写入模板仍要求工具或命令声明 writes 并通过宿主权限检查。模板契约不是操作系统沙箱；私有目录限制的是 API 路径及工作目录选择，不能限制已授权程序自身的系统调用。

默认超时 30 秒，私有目录进程最多 1800 秒，并受整个回调时限约束。普通目录进程保持 120 秒上限。两种入口使用同一进程树回收实现和有界输出规则。为覆盖构建与安装多个步骤，清单的回调总时长硬上限为 3600 秒，缺省仍为 20 秒。

Windows 的进程工作目录仍受[系统长度限制](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setcurrentdirectory)。超限时宿主查询已有的[短路径](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getshortpathnamew)，并确认它仍指向授权目录，再传给子进程。没有可用短路径的超限目录会明确返回错误。该转换不改变全局工作目录，也不修改卷上的短文件名设置。

## AUR 插件的业务约束

`package-advisor` 示例提供 `lua__package-advisor__review_aur_package` 和 `lua__package-advisor__install_aur_package`。前者只读，后者声明 writes；安装默认禁用且没有授权，设置与启用状态由普通插件配置管理。旧主配置不再提供默认值。

审查优先使用可用的 paru，其次 yay，两者都不可用时才下载官方快照；选中的助手执行失败即停止，不再尝试其他助手。审查不执行 PKGBUILD。风险模式、深度两层、80 个文件和每文件 24000 个 Unicode 字符沿用原设计。截断或无法读取的证据会使 `review_complete=false`，禁止安装。新审查开始即撤销旧许可，报告超过输出预算时不留下可安装记录。旧版全局 `aur-review-state.json` 不会迁入新的会话状态，需要重新审查。

安装要求 `user_confirmed=true`、Linux 平台、完整且允许安装的审查，以及不同的 `ctx.operation_id`。同一次 Agent 请求及其插件组合调用共享操作标识，因此创建新 Lua VM 不能绕过分轮检查。布尔参数仍由调用方依据用户回复填写；它不解析或证明用户回复内容，实际写入继续依赖宿主权限流程。

审查记录通过比较交换支持一次安装尝试。助手安装失败、构建失败或取消后再次安装需要重新审查；同一记录不能重复启动安装。安装同样选择首个可用的 paru 或 yay，选中后失败即停止；两者都不可用时才采用快照、makepkg 和 pacman -U，排除签名文件作为安装产物。安装阶段仍可能重新下载可变的远端内容，当前记录不是对安装字节的签名或固定快照承诺。
