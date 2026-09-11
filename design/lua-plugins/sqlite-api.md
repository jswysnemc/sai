# SQLite 快照接口

`sai.sqlite` 在内存中查询或修改完整 SQLite 镜像。输入和输出复用[二进制缓冲](binary-api.md)，不接收数据库路径、SQL 文本、表达式或执行函数。表结构、数据转换、搜索评分、分块和嵌入策略由插件实现。

## 调用与授权

```lua
sai.sqlite.query(snapshot, query)
sai.sqlite.apply(snapshot_or_nil, changes, options_or_nil)
```

仅在执行回调中开放，初始化期间调用会失败。纯内存计算无需外部能力授权，可以在只读回调中使用；清单不增加数据库专属能力。每次通过参数校验并取得工作额度的操作消耗一次共用 `system_calls`，数据库错误和超时也计数。

`snapshot` 必须是本实例、本回调中尚未关闭的二进制缓冲。保存到全局变量不能延长使用期限；回调结束后句柄失效。`apply(nil, ...)` 创建新数据库，`query` 不接受 `nil`。

文件访问仍使用已有授权：

| 行为 | 所需能力 |
| --- | --- |
| `sai.binary.read_file(path)` | `system.read_paths` |
| `buffer:write(path)` | `binary.write_paths` 和可信写入许可 |
| `buffer:write_if(path, expected)` | `system.read_paths`、`binary.write_paths` 和可信写入许可 |
| 查询或修改内存快照 | 无外部能力；受回调、计算和二进制预算限制 |

改变 `ctx.allow_writes`、`ctx.workdir` 或 `sai.limits` 不能扩大实际权限或预算。只读工具能创建修改后的内存镜像，发布到文件时仍会检查写入权限。

## 创建和查询

以下示例放在工具或用户命令的执行函数中：

```lua
local snapshot = sai.sqlite.apply(nil, {
    {
        op = "create_table",
        name = "notes",
        columns = {
            {name = "id", kind = "integer"},
            {name = "text", kind = "text", nullable = false},
            {name = "score", kind = "real"},
        },
        primary_key = "id",
    },
    {
        op = "insert",
        table = "notes",
        rows = {{id = 1, text = "内容", score = 0.5}},
    },
}, {max_bytes = 65536})

local result = sai.sqlite.query(snapshot, {
    table = "notes",
    columns = {"id", "text", "score"},
    where = {id = 1},
    order_by = {{column = "id", descending = false}},
    limit = 100,
    offset = 0,
})
snapshot:close()
return result
```

结果为 `{columns, rows}`。`columns` 保留请求顺序；每行按请求列名映射到值。没有匹配记录时 `rows` 是空数组。查询默认最多返回 100 行；没有 `order_by` 时不保证行顺序。

`where` 为列到标量的映射，全部条件按 AND 组合，使用 SQLite `IS` 比较；`sai.json.null` 可以匹配 SQL NULL。缺省或空过滤器匹配全部记录。只支持单表投影、等值过滤和排序分页，不提供模糊匹配、聚合、联表或 SQL 表达式。查询不使用附属索引表达式，执行过程仍受指令预算限制。

## 批量变更

`changes` 是非空数组。先校验整个请求，再在独立内存副本中执行单个事务，全部成功后返回新的二进制缓冲。原缓冲始终不可变；任一约束、结构、预算或导出错误都不返回部分结果。

| `op` | 字段 | 行为 |
| --- | --- | --- |
| `create_table` | `name`、`columns`、可选 `primary_key`、`auto_increment` | 创建普通表；列包含 `name`、`kind`、可选 `nullable` |
| `create_index` | `name`、`table`、`columns`、可选 `unique` | 创建普通索引，可声明唯一约束 |
| `insert` | `table`、`rows` | 按字段映射逐行插入；约束冲突使整批失败 |
| `upsert` | `table`、`key`、`rows` | 按已有主键或唯一约束处理冲突，只更新本次提供的其他字段 |
| `delete` | `table`、可选 `where` | 删除等值条件匹配的记录；省略或空过滤器清空表 |

列的 `kind` 只接受 `integer`、`real`、`text`，使用 SQLite 类型亲和性，不创建 STRICT 表。`nullable` 默认 `true`。`primary_key` 是已声明的单列名称；`auto_increment` 默认 `false`，仅允许与整数主键组合。未提供值的列遵循表自身约束。

`create_table` 和 `create_index` 使用 `IF NOT EXISTS`：已有同名对象不会被重新定义，也不会自动补列、改类型或修正索引。它们不构成模式迁移接口。创建后重新检查结构对象总数，不能通过多次批量新增绕过限制。

每个 `upsert` 行必须提供非 null 的冲突键；表中必须已有对应主键或唯一约束。只提供键时，冲突记录保持不变。主键以外未提供的字段保持原值。

输入标量支持 null、布尔值、文字、Lua 整数和有限实数。布尔值作为 0 或 1 绑定；整数使用有符号 64 位范围，实数使用双精度。NaN、无穷大、嵌套字段值及数组字段值会失败，不能隐式转换为空值。需要校验模型参数的原始整数表示时使用 [`ctx.json_integer`](api.md#上下文与状态)，不要依赖浮点数恢复精度。

查询保留 SQLite 实际值类型。BLOB、非法 UTF-8、非有限实数和超过单值限制的文字会使完整查询失败；不会替换、截断或跳过问题行。

## 文件条件发布

数据库文件所在目录须分别声明并授予读取和输出权限，例如：

```json
{
  "capabilities": {
    "system": {"read_paths": ["data/indexes"]},
    "binary": {"write_paths": ["data/indexes"]}
  }
}
```

```lua
local original = sai.binary.read_file("data/indexes/notes.db", {max_bytes = 65536})
local expected = original:sha256()
local updated = sai.sqlite.apply(original, {
    {op = "upsert", table = "notes", key = "id", rows = {{id = 1, text = "新内容"}}},
}, {max_bytes = 131072})
original:close()
local published = updated:write_if("data/indexes/notes.db", expected)
updated:close()
return {published = published}
```

新建文件使用 `write_if(path, nil)`。修订冲突返回 `false`，由插件决定是否重新读取和计算；其他错误不会伪装成冲突。快照本身没有文件关联，`apply` 成功并不表示磁盘已经更新。

条件比较、暂存和发布沿用[文件修订契约](binary-api.md#文件修订与条件写入)。新版插件输出共用同一应用状态目录中的锁；外部编辑器、原生 SQLite 连接和其他状态目录的写入不参与互斥。文件输入必须是完整一致的独立镜像，不能把对活动数据库文件的普通读取视为 SQLite 在线备份。发布完成后，后续取消不能回滚已写入的文件。

## 镜像和结构限制

输入必须包含合法 SQLite 文件头、页大小和完整页，页数不得超出实际容量。拒绝 WAL 格式镜像，因为接口不读取关联 WAL 文件。已有 WAL 数据库需要先完成检查点并切换到回滚日志格式，例如由数据库所有者在适当时机执行 `wal_checkpoint` 和 `journal_mode=DELETE`；仅做检查点不改变 WAL 格式标志。插件接口不能执行这些 SQL 命令。

最多允许 128 个表和索引，SQLite 自建的序列表和自动索引也计数。数据库中任何视图、触发器或虚表都会导致拒绝；访问的目标表还不能包含生成列或外键。普通记录、主键、非空约束、唯一索引和自动编号可以保留。

表名、索引名和请求列名只允许 1–64 字节 ASCII 字母、数字及下划线，首字符不能为数字，也不能使用不区分大小写的 `sqlite_` 前缀。列集合拒绝大小写重复名称。标识始终引用，值始终参数绑定；未知字段和动作直接失败。

## 资源和取消

| 项目 | 边界 |
| --- | --- |
| 独立数据库镜像硬上限 | 64 MiB，实际还受可用二进制和计算预算限制 |
| `apply` 最小有效容量 | 8192 字节，且不得小于输入镜像 |
| 结构化参数和查询 JSON | 分别不超过 `min(limits.output_bytes, 1 MiB)`，按转义后完整字节计算 |
| 单个文字参数或结果 | 256 KiB |
| 一批变更 | 1–64 项 |
| 一批 `insert` / `upsert` 合计 | 最多 512 行，单项行数组不能为空 |
| 请求列、过滤列或排序列 | 每组最多 32 列 |
| 查询 `limit` | 1–512，默认 100 |
| 查询 `offset` | 0–1,000,000，默认 0 |
| 请求嵌套 | 最多 8 层；循环表不能进入解码 |

设输入镜像大小为 A，查询输出上限为 O，修改容量为 C，固定工作余量为 1 MiB：

- 查询除已持有的 A 字节外，再预留 `2 × A + 1 MiB + O`。
- 修改除已有输入外，预留 `2 × C + 1 MiB` 工作额度与 C 字节输出额度。
- `apply` 默认 C 为 `max(2 × A, 1 MiB)`，新建时为 1 MiB。`options.max_bytes` 可指定上限；实际容量继续收窄到硬上限及当前可用额度允许的范围。不足 8192 字节或不足以容纳输入时失败。
- 输出成功后只按实际镜像长度持有额度。关闭缓冲或回调结束释放句柄引用；仍在执行的工作保留自己的引用和预留，直到线程退出。

这些预留限制同一 VM 的资源占用和并发计算。数据库主体分配为固定容量，SQLite 不得自动扩大镜像缓冲；SQLite 其他内部结构、Lua 堆与整个 Sai 进程的内存不能等同于这个额度。原生连接还限制缓存、字段数、SQL 长度、表达式深度和程序规模，临时数据只在内存中处理。

请求检查、镜像初始化与导出按数据量计费；SQLite 每 512 条虚拟机指令扣除共用执行预算，并检查取消与回调截止时间。修改 `sai.limits.instructions` 不能改变实际额度。首个原生预算错误保持到本次操作结束，不因 SQLite 重试或后续检查而消失。

两种操作均受 `limits.binary_timeout_ms` 及外层回调期限约束，工作线程排队也计时。`apply` 的 `options.timeout_ms` 可进一步收窄时限；零或非法值拒绝，较大值收窄到清单上限。`query` 不接收第三参数。局部超时可以由 Lua 捕获；取消后不给当前回调交付迟到结果，也不提前归还仍被线程占用的额度。

## 实现边界

[运行时绑定](../../crates/sai-plugin-runtime/src/runtime/binary/sqlite.rs)处理句柄、请求、配额和异步取消，[数据库模块](../../crates/sai-plugin-runtime/src/sqlite/mod.rs)处理结构化查询和事务。连接启用 defensive、关闭 trusted schema 和视图、安装授权器，并对查询使用只读反序列化。

[固定缓冲库](../../crates/sai-sqlite-buffer/src/snapshot.rs)封装 SQLite 分配、反序列化所有权和借用导出所需的 FFI。它在构造失败时释放未移交的分配，移交后由 SQLite 管理释放；导出独占借用数据库并分块复制。插件运行时保留 `forbid(unsafe_code)`，不直接操作裸指针。

当前接口已验证原知识库的 `files`、`semantic_chunks`、普通索引及自动编号结构。知识库工具、管理命令、搜索、嵌入请求和后台重建仍由原实现提供，完整业务迁移见[迁移进度](migration.md)。
