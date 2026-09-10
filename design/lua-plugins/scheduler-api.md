# 插件持久调度接口

`sai.scheduler` 在指定 Unix 秒执行本插件已注册的用户命令。任务记录和独立工作进程使创建命令退出后仍能执行；调度、通知投递和业务参数解析分别由不同接口负责。

## 声明与授权

```json
{
  "capabilities": {
    "system": {
      "schedule": true
    }
  }
}
```

外部插件默认没有调度权限，使用以下命令单独调整：

```sh
sai plugins enable my-plugin --allow-schedule
sai plugins enable my-plugin --no-schedule
sai plugins info my-plugin --json
```

两个开关互斥，也不能与 `--grant-declared` 同用；未指定的其他授权保持原值。清单未声明 `system.schedule` 时不能授予该能力。通知、模板进程和插件存储授权不会隐式授予调度权限。

所有接口都要求有效的 `system.schedule`。创建、取消和恢复还要求当前工具或命令取得可信写入权限；`optional_writes` 工具在只读调用中只能查询。事件可以查询，不能创建、取消或恢复。初始化阶段不能访问任务记录；通知纯展示运行时不继承调度权限。

## 创建与查询

以下代码在已授权的写入回调中使用：

```lua
local task = sai.scheduler.schedule({
    due_at = sai.time.now() + 30,
    command = "deliver",
    arguments = "需要交给命令的文本",
})
local current = sai.scheduler.get(task.id)
local tasks = sai.scheduler.list({offset=0, limit=16})
```

| 接口 | 返回值 |
| --- | --- |
| `schedule(request)` | 完成发布的任务记录 |
| `list(options?)` | 本插件任务数组，不启动或恢复任务 |
| `get(id)` | 单条任务；不存在时为 `sai.json.null` |
| `cancel(id)` | 是否接受取消请求；不存在或已经终结时为 `false` |
| `resume(id)` | 恢复或确认中断后的任务记录 |

`schedule` 只接受以下字段，未知字段报错：

| 字段 | 约束 |
| --- | --- |
| `due_at` | 必填整数 Unix 秒，范围 `0..=253402300799`，即 1970–9999 年；过去时间尽快执行 |
| `command` | 必填，本插件已注册命令的包内名称，最多 48 字节；使用与命令注册相同的标识规则 |
| `arguments` | 可选 UTF-8 字符串，默认空文本，最多 16 KiB；原样传给命令 |

不能指定其他插件、程序路径、工作目录、会话或授权。宿主在创建时读取当前包，确认命令存在及调用实例仍与磁盘配置一致。

`list()` 等价于 `list({offset=0, limit=16})`。`offset` 为 0–128 的整数，`limit` 为 1–16 的整数，未知选项报错。结果按创建时间、任务 ID 升序排列；并发新增和历史清理可能改变后续分页位置，各页不构成同一事务快照。

## 任务记录与状态

任务 ID 为 `job-` 加 32 位小写十六进制 UUID。其他插件无法通过这个 ID 查询或取消记录。

| 字段 | 含义 |
| --- | --- |
| `id`、`command`、`arguments`、`due_at` | 任务身份及原始请求 |
| `status` | 下表中的持久状态 |
| `pid` | 工作进程标识，用于显示与启动握手；未发布时为 JSON null |
| `created_at` | 创建时的 Unix 秒 |
| `finished_at` | 终结时的 Unix 秒；活动任务为 JSON null |
| `output` | 成功命令的文本结果；没有结果时为 JSON null |
| `output_truncated` | 持久输出是否经过截断 |
| `error` | 失败原因；没有错误时为 JSON null |

成功输出最多保留 16 KiB，错误最多保留 4096 字节，均在 UTF-8 字符边界截断。命令本身先受运行时输出预算限制，超出该预算属于执行失败；持久记录的截断只处理已经成功返回的文本。

| 状态 | 含义 |
| --- | --- |
| `scheduled` | 已创建，等待工作进程和到期时间 |
| `running` | 已记录开始执行；命令仍受清单时限与资源预算约束 |
| `cancelling` | 已请求取消，等待工作进程释放命令调用 |
| `cancelled` | 取消已记录，任务不再执行 |
| `completed` | 命令成功完成 |
| `failed` | 启动、授权复核或命令执行失败，或者已开始的工作进程中断 |

状态是最后一次持久提交的结果，不是实时进程存活证明。进程异常退出后，记录可能仍显示 `scheduled`、`running` 或 `cancelling`，查询不会自动修正或重新执行。

每插件最多 32 个活动任务，`scheduled`、`running` 和 `cancelling` 都占用额度；最多保留 128 条记录。创建新任务达到历史上限时，只清理按创建时间排序的最旧终态记录，不驱逐活动任务。单条内部记录最多 256 KiB；损坏、超限、归属不符及特殊文件均报错，不会静默跳过以绕过配额。

## 取消与恢复

```lua
local accepted = sai.scheduler.cancel(task.id)
local recovered = sai.scheduler.resume(task.id)
```

取消只修改本插件任务状态，不根据持久 PID 发送终止信号。未开始的任务直接记录为 `cancelled`；执行中的任务先进入 `cancelling`，工作进程观察状态后释放命令 Future，再记录终态。执行锁已经释放时，可以直接确认取消。

`resume` 要求原工作进程不再持有执行锁：

- `scheduled` 任务重新验证当前插件、授权与版本摘要，保留原任务 ID、参数和到期时间，生成新的启动身份；过期任务尽快执行。
- `running` 任务记录为 `failed`，明确说明中断后未重试；不会再次进入命令。
- `cancelling` 任务确认成 `cancelled`。
- 已终结任务、缺失记录或仍有执行锁的任务返回错误。

宿主不在开机或普通 Sai 启动时自动扫描和恢复。创建命令正常退出后，已经发布的独立工作进程继续等待；机器重启或工作进程异常退出需要显式恢复。调度不保证断电后的必达，也不把命令外部副作用纳入原子事务。已经投递的通知、完成的文件写入及其他外部结果不能回滚，已开始的任务不会自动重试。

创建操作在阻塞线程排队期间取消后，线程不能迟到发布任务；进入发布事务后发生取消、超时或返回值超限，已经提交的持久副作用不保证撤销。调用方遇到不确定结果时可以先查询任务，避免直接重复创建。

## 后台执行边界

创建时绑定插件 ID、可信工作目录、宿主路径和完整版本摘要。摘要覆盖清单、Lua 源码、设置与授权；内部记录只保存摘要、路径、任务参数和有界结果，不复制插件设置或凭据。公开任务不包含宿主路径和摘要。

到期执行重新读取当前配置与包，要求插件仍可用、已启用、具有调度授权，且完整摘要与创建时一致。撤权、禁用、移除、源码或设置变化会使任务拒绝执行；恢复也执行同一检查。已经加载并开始的命令使用自己的快照，后续配置变化不替代显式取消。

后台命令使用独立 Lua 实例，`ctx.session_id` 为 `scheduled/<任务 ID>`，`ctx.operation_id` 为任务 ID，工作目录沿用创建时的可信目录。命令自身的 `access` 声明仍生效，只读命令不能写入存储或发送通知；修改公开 `ctx` 不能扩大权限。调度授权不自动提供通知、文件、网络等其他能力。

后台调用不附加 Agent 模型和工具调用服务，不能依靠 `sai.model.complete` 或 `sai.tools.call` 接续原会话。等待到期不占命令回调时限；到期加载与执行继续遵守清单内存、指令、时长和输出限制。

Unix 工作进程通过 `setsid` 脱离父终端会话；Windows 使用独立进程组和分离标志。宿主只启动当前 Sai 程序的内部工作入口，参数使用独立 argv。随机启动身份、PID 发布握手与执行锁共同拒绝重复或过期入口。

记录存放在 Sai 状态目录的 `plugin-jobs` 类别，按插件隔离；创建、配额与状态转换使用短操作锁和原子文件替换。工作进程另持整个执行期间的独占锁。公开操作遇到短锁占用时返回可重试错误。读取和取消不需要加载插件代码，禁用、卸载与会话重置保留历史记录。

所有调度操作与环境、文件、进程、存储和通知共用 `limits.system_calls`。无效参数和运行时授权失败不计数；进入宿主后的失败仍计数。返回任务和分页结果还需满足 `limits.output_bytes`，可以通过减小分页数量避免列表结果超限。

## 管理命令

```sh
sai plugins jobs my-plugin list --json
sai plugins jobs my-plugin cancel job-0123456789abcdef0123456789abcdef --json
sai plugins jobs my-plugin resume job-0123456789abcdef0123456789abcdef --json
```

三个管理操作都输出 JSON，分别为 `{"jobs":[...]}`、`{"id":"...","cancelled":true}` 和 `{"task":{...}}`。管理列表可以读取全部 128 条记录。列表和取消不要求插件仍然启用或安装，恢复待执行任务必须重新通过授权与版本检查。管理操作无需模型服务。

## 与通知组合

通知命令需要同时声明并获得 `system.schedule` 与 `system.notify`。以下入口只注册命令，实际创建发生在写入回调中：

```lua
--- 【提醒示例】【投递】把调度参数交给宿主桌面通知
--- @param arguments string 提醒正文，最多 4096 字节
--- @return table 已完成的通知通道
local function deliver(arguments)
    return sai.notify.send({title="Sai", body=arguments})
end

--- 【提醒示例】【创建】安排三十秒后执行本插件通知命令
--- @param arguments string 提醒正文，最多 4096 字节
--- @return table 已发布的持久任务
local function remind(arguments)
    assert(#arguments <= 4096, "提醒正文超过 4096 字节")
    return sai.scheduler.schedule({
        due_at=sai.time.now() + 30,
        command="deliver",
        arguments=arguments,
    })
end

sai.register_command({
    name="deliver", description="Deliver a reminder.",
    access="writes", execute=deliver,
})
sai.register_command({
    name="remind", description="Schedule a reminder in thirty seconds.",
    access="writes", execute=remind,
})
```

```sh
sai plugins enable my-plugin --allow-schedule --allow-notify
sai plugins run my-plugin remind "检查任务结果"
```

声音、本地音频读取和通知取消边界见[主动通知接口](notification-api.md)。本接口提供通用调度能力；原闹钟工具的时间解析、旧记录兼容与 Lua 业务迁移尚未完成。
