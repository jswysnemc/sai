# 会话待办插件

`examples/lua-plugins/todo` 提供 `lua__todo__todo`、计划历史和工具循环提醒。清单增删改查、顺序校验、显式旧数据导入及归档均由 Lua 执行；Rust 负责公共存储、可信会话归属和回调分发。安装与授权见[示例说明](../../examples/lua-plugins/todo/README.md)。

## 模块职责

| 文件 | 职责 |
| --- | --- |
| `sai-plugin.json`、`init.lua` | 能力、预算、语言校验和入口注册 |
| `definition.lua` | 原中英文工具说明与 JSON Schema |
| `arguments.lua` | Unicode 空白、批量文字、原始整数与目标定位 |
| `transitions.lua` | 状态名称、前置条目和唯一进行中项校验 |
| `state_schema.lua` | 记录结构、重复 ID、旧数据转换和 JSON 值比较 |
| `state.lua` | 完整记录比较交换、冲突重试、归档与快照 |
| `import.lua` | 显式快照导入、来源校验和禁止覆盖非空计划 |
| `actions.lua` | 四个公开动作、批量创建和竞争中的目标绑定 |
| `reminder.lua` | 显式循环状态、连续未更新计数与原提醒文案 |

## 工具与命令

包内工具名为 `todo`，对外名称为 `lua__todo__todo`，保留原双语说明、Schema 和结果字段。整个工具声明为 `writes`，包括 `list` 动作；计划模式不公开它，网关也沿用原工具目录限制。

| 动作 | 参数与行为 | 返回值 |
| --- | --- | --- |
| `list` | 查询活动清单，整批结束时先归档 | `{ok=true, items}` |
| `add` | 非空 `texts` 优先，否则使用 `text`；可用 `index` 插入 | `{ok=true, changed, items}` |
| `update` | 非空 `id` 优先，否则使用 `index`；修改 `text` 或 `status` | `{ok=true, changed, items}` |
| `remove` | 按同样规则定位并移除一项 | `{ok=true, changed, items}` |

文字按原规则去除两端 Unicode 空白。序号从 1 开始；新增时 0 插到首位，缺省或超过清单长度时追加。更新或删除的 0、超界值明确报错；指定 ID 不存在时不回退到序号。原始整数通过 `ctx.json_integer` 处理，避免大整数经 Lua 浮点数转换后定位错误。

状态只接受 `pending`、`in_progress`、`completed`、`cancelled`。推进为进行中或已完成前，所有前置条目必须完成或取消；最多一项处于进行中。整批没有未完成项时，清空活动清单并追加 `{archived_at, items}` 历史。条目保留 `id`、`text`、`status`、`created_at`、`updated_at`；批量新增的毫秒 ID 和 UTC 文本来自同一次 `sai.time.utc_now()`。

运行时先校验公开 Schema，再进入 Lua。旧原生实现曾容忍但违反 Schema 的输入现在明确拒绝；正常输入保持原业务规则与结果，动态 ID、时间及外层错误封装不承诺逐字节相同。

包另注册只读 `snapshot` 命令，返回 `{items, history}`。CLI 可执行 `sai --plan plugins run --session default todo snapshot`，TUI 可执行 `/plugins run todo snapshot`。快照保留空状态初始化与完成归档语义，因此查询可能提交公共会话存储记录；它不自动导入旧文件，也不提供增删改参数。

`plugins call` 和 `plugins run` 的 `--session` 参数解析当前工作区的真实会话目录，与 Agent 和 Web 视图共用记录。省略时使用 `plugin-command/<规范工作目录>` 作用域，各工作区互相隔离。默认工具目录不包含未安装的待办，旧短名称不再注册。

## 授权与配置

```json
{"capabilities":{"reply_policy":true,"system":{"session_storage":true}}}
```

```sh
sai plugins enable todo --allow-session-storage --allow-reply-policy
sai plugins enable todo --no-reply-policy
sai plugins disable todo
```

会话存储和回复策略分别授权。撤销回复策略后工具仍可使用；撤销存储后直接工具和快照调用明确失败，可选 Web 视图返回空数据。禁用或撤权不删除记录，恢复授权后可继续读取。独立 `language` 设置只接受 `en` 或 `zh`，默认 `en`；未知字段拒绝保存。

## 会话文件与旧数据

插件通过公共 `sai.storage` 的 `plan` 键保存完整记录，作用域来自可信会话目录或直接命令上下文。宿主不按 `todo` ID 注入特殊路径或旧文件种子。

```json
{"version":1,"items":[],"history":[]}
```

`import` 是显式写入命令，接收 `{"state":{...}}` 或 `{"path":".sai/todo-import/snapshot.json"}`，二者必须选一。支持版本 0 和 1；文件来源另需清单声明及读取授权。导入验证条目、历史、重复 ID 和 256 KiB 上限，再以比较交换提交。当前条目或历史非空时禁止覆盖，空快照初始化不阻止导入。旧分离文件需要先明确组装完整状态。

卸载、禁用和撤权保留公共计划。`StateStore::reset_conversation()`、清空对话及删除会话会按公共会话存储规则清除当前计划和历史。清空对话不改写旧 `todos.json`、`todos.history.json`、`todos.plugin.json`，仍可显式导入。整体清空会话数据或删除会话会删除会话目录，目录内的旧文件也随之移除，需要保留的备份应存放在会话目录外。其他会话记录与跨会话存储遵循各自生命周期，详见[私有状态接口](private-api.md)。

公共存储文件、锁及逐层会话目录拒绝符号链接；导入文件也必须通过普通文件读取检查。损坏 JSON、非法结构、重复 ID、特殊文件和超限记录均明确失败，不以空清单覆盖。导入不会改写来源文件。

## 原子更新与预算

活动项与全部历史保存在同一条最多 256 KiB 的 JSON 记录中，比较交换按 JSON 值比较，不受对象字段顺序影响。单次业务操作最多尝试 16 次比较交换；不匹配时重新读取并合并。更新或删除首次按序号找到条目后固定其 ID，后续重试不会误操作移动到原序号的其他条目。同一条目的不同字段更新可以合并，完成最后一项的竞争只归档一次。

正式宿主与普通会话存储共用 `.plugin-state.lock`，锁覆盖读取、比较和原子发布。锁忙立即返回错误，不纳入 Lua 的比较不匹配重试；调用方需重新提交操作。16 次比较均不匹配时明确报告状态变化过于频繁。归档历史没有额外的自动裁剪规则，达到完整记录上限时拒绝提交。

包使用 16 MiB Lua 堆、400 万条指令、10 秒回调时限和每回调 128 次系统操作。输出上限为 512 KiB，因为正常结果可能同时包含 `changed` 和 `items`；存储上限仍为 256 KiB。

同目录暂存、同步与原子替换保证活动项和历史一起提交。已完成的文件发布不能随外层取消回滚；调用失败或取消后，应在后续有效调用中读取状态确认。旧程序、外部编辑器和其他状态目录不参与这把锁，不能把它视为任意外部文件修改之间的事务。

## 工具循环提醒

`reminder.lua` 使用[回复策略的 `after_tool` 回调](reply-policy-api.md)。每个工具循环从空状态开始，存在未完成项且连续三个工具调用没有成功增删改时，追加原中文提醒；每个循环最多一次。成功的 `add`、`update`、`remove` 重置连续计数，`list` 和失败调用不算更新。读取没有未完成项时同样重置计数。

回调沿用原提醒接入位置，只处理普通串行工具完成分支；并发只读组、门禁提前拒绝和特殊分支不触发它。当前目录未提供 `lua__todo__todo`、计划模式、插件禁用或回复策略撤权时不提醒。回调只读检查当前记录，不导入或归档旧文件；错误、超时和损坏状态不会改变原工具结果或主回复终态。

## Web 查询

`GET /api/todos` 按当前配置执行普通安装包的 `snapshot`，返回原 `{items, history}`。`GET /api/session-data` 按各会话完整目录补充待办数量，并在快照查询后重新统计会话目录文件。公共插件记录保存在独立命名空间，不计入会话目录字节数。

未安装、禁用或存储撤权时快照为空、数量为 0。已授权状态下的记录损坏或链接使查询失败；会话数据列表保留其他可用指标，以 `todo_count=null` 和 `state_error` 表示对应会话错误。插件配置、目录或源码损坏仍明确报错。两个入口不维护第二份 Rust 业务规则，前端继续使用原 JSON 字段。

## 验证依据

冻结提交 `9daf3fe` 的原业务源码形成 989 条业务样本、共 1000 次调用，以及 127 条提醒序列。966 条符合 Schema 的样本比较结果、活动项、归档和原错误内容，仅规范动态 ID 与时间；23 条违反 Schema 的样本断言拒绝且状态不变。另比较完整中英文工具定义。

内置阶段的回归与发布证据保留在[第二十三轮迁移记录](migration.md#第二十三轮验证记录)。当前测试改为普通安装包、公共存储和显式导入，另覆盖指定会话的 CLI 调用、工作区隔离、撤权空视图以及重置后的旧文件保留；本次提取验收见[归档状态](../../.doc/archive/2026-09-14-core-plugin-extraction/status.md)。
