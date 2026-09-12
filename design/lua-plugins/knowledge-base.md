# 本地知识库插件

`plugins/knowledge-base` 实现知识文件导入、编辑、删除、关键词与语义搜索，以及后台嵌入重建。六个工具保留原公开名称、完整描述和参数 Schema；`sai kb` 与配置 TUI 调用同一 Lua 包。原 `src/tools/knowledge_base.rs` 和 `src/tools/knowledge_base/` 业务实现已删除。

## 文件职责

```text
plugins/knowledge-base/
├── sai-plugin.json                 能力与资源限制
├── init.lua                        实例装配
├── defaults.lua、settings.lua       默认值与配置校验
├── definitions.lua、tools.lua       原工具契约与调用分派
├── command_args.lua、commands.lua   管理参数校验与命令组合
├── values.lua、paths.lua            数值兼容、文字和相对路径规则
├── storage/
│   ├── schema.lua                  两个原数据库的结构
│   ├── snapshot.lua                有界快照、分页和条件发布
│   ├── records.lua、read.lua        元数据与完整正文读取
│   └── transaction.lua             私有锁和未完成写入恢复
├── mutations/
│   ├── source.lua、import.lua       显式来源导入与聊天上传
│   └── edit.lua、remove.lua         行编辑和删除
├── search/
│   ├── tokens.lua、snippets.lua     分词、命中位置和片段
│   ├── keyword.lua、semantic.lua    两种评分及结果合并
│   └── run.lua                     关键词与网络查询编排
└── embedding/
    ├── client.lua、chunks.lua       嵌入协议、响应校验和分块
    └── jobs.lua、reindex.lua        队列合并与完整块发布
```

Rust 的 `src/cli/kb_commands.rs` 只转换原命令参数；`src/plugins/knowledge_view.rs` 只提供 TUI DTO 和命令桥接。`src/plugins/commands.rs` 执行可信内置命令，`compatibility/knowledge_base.rs` 负责旧配置、当前嵌入供应商及最小能力投影。文件、锁、SQLite 和持久调度宿主不包含知识库表名或评分规则。

## 工具与管理入口

| 工具 | 访问类型 | 行为 |
| --- | --- | --- |
| `search_knowledge_base` | 只读 | 关键词优先，按条件尝试语义搜索并合并评分 |
| `search_knowledge_base_by_name` | 只读 | 完整路径、文件名和部分词元评分 |
| `read_knowledge_base_file` | 只读 | 按行读取，保留越界与继续阅读说明 |
| `upload_text_to_knowledge_base` | 写入 | 保存带来源、标题和本地上传时间的 Markdown |
| `edit_knowledge_base_file` | 写入 | 使用从 1 开始的行区间替换或删除正文 |
| `remove_knowledge_base_file` | 写入 | 删除正文、元数据和该文件的语义块 |

只读工具不创建知识库目录或索引。上传开关同时控制三项写入工具；管理命令继续独立可用。上传保留原用途检查，拒绝把技能、记忆、身份、提示词或配置请求作为知识资料保存。

```sh
sai kb add ./notes
sai kb list
sai kb search Rust --limit 5
sai kb find notes --limit 5
sai kb read notes/example.md --start 1 --lines 100
sai kb remove notes/example.md
sai kb reindex
sai kb stats
sai kb embed reindex --quiet
```

对应插件命令为 `add`、`list`、`search`、`find`、`read`、`remove`、`reindex`、`stats`、`embed-reindex`。它们接收 JSON 对象；`add` 和 `list` 支持 `format="json"`，供配置 TUI 使用。`limit`、`start`、`lines` 接受非负整数或完整十进制字符串，最大为 u64；旧 CLI 把这些参数转换为十进制字符串，避免 Lua 浮点转换丢失精度。未知字段、错误类型与越界值在文件操作前拒绝。

原管理查询会初始化目录，因此九项命令都声明写入权限，并可恢复未完成记录。显式 `--plan` 拒绝这些管理命令；只读工具仍可在计划模式查询现有完整库。配置 TUI 保留原菜单和删除确认，进入页面或完成变更后才刷新数据。

## 配置与权限

旧 `plugins.knowledge_base.enabled` 提供缺省启用状态，显式插件启停优先。旧业务设置作为默认值，`plugins.jsonc` 中的包设置按字段覆盖。空 `data_dir` 使用应用数据目录下的 `kb`；相对目录按本次可信工作目录解析。

| 能力 | 最小范围 |
| --- | --- |
| `system.read_paths` | 知识库根目录，以及最多 32 项显式 `input_paths` |
| `binary.write_paths` | 知识库根目录 |
| `system.remove_paths` | 知识库根目录，包含正文和恢复记录删除 |
| `system.plugin_storage` | 本插件锁与小型后台队列记录 |
| `system.schedule` | 本插件后台重建命令的创建和查询 |
| `http`、`http_read_only_post` | 当前嵌入供应商来源和精确 `/embeddings` 端点 |

`sai kb add` 和配置 TUI 只为用户明确选择的来源增加单次读取授权，不保存到插件设置，也不交给后续后台任务。普通 `plugins run knowledge-base add` 必须已有来源读取授权。索引中的绝对 `path` 不作为实际读取来源；Lua 始终从经过校验的库内相对名称生成路径。

供应商投影只包含当前选择的 ID、端点和已解析密钥，兼容单密钥、多密钥选中项、环境引用和私密配置。派生凭据不写入插件配置或调度参数，外部同 ID 包不能继承。显式目录及 HTTP 授权保持固定，更改库路径或端点后仍须满足当前授权交集。

## 原索引与搜索兼容

继续使用 `files/`、`kb_meta.db` 的 `files` 表和 `semantic_index.db` 的 `semantic_chunks` 表。普通索引、自动编号、旧行及向量可直接读取；只读操作保持数据库原字节。写入入口可以补齐空数据库或缺失表，完整结构不重写。损坏、超限或 WAL 格式镜像不会当成空库覆盖；WAL 的处理要求见 [SQLite 快照接口](sqlite-api.md)。

关键词分词、ASCII 大小写、中文二元词、字节位置、邻近窗口和片段规则保留原行为。语义评分逐步按 f32 舍入，名称评分保留原 f64 表示，同分条目按原顺序稳定排序。强关键词命中跳过 HTTP；语义请求或扫描失败时沿用关键词回退，不把 `semantic_used=false` 解释为索引一定为空。

语义查询默认每页 32 行，减少重复复制完整数据库。只有单页 JSON 总量超限时才减半页大小，并在原偏移重试；单行限制、损坏结构、取消和预算错误继续传播。分页不会交付半页结果，也不提高全局指令上限。

## 写入与恢复

同一应用状态根目录下的本插件调用共享 `knowledge:store` 私有锁。每次变更先保存 `pending-write.json`，随后更新正文、元数据与所需语义清理，全部成功后删除恢复记录。记录保存操作、规范相对名称、预期 SHA-256、正文和清理标志，恢复前重新校验这些字段。

正文通过原修订条件发布。恢复写入拒绝覆盖外部新内容；恢复删除也复核原摘要，拒绝删除中断后重新创建或修改的文件。失败或取消可能发生在部分文件已经提交之后；只读入口遇到恢复记录会明确拒绝，并提示执行 `sai kb reindex`。后续具备写入许可的管理或变更入口可以继续幂等恢复，权限不足时保留记录。

这套恢复协议不提供跨多个文件的 ACID 事务。外部编辑器和旧原生进程不参与新私有锁，不能保证与它们并发编辑时的一致性；条件发布和提交复核只覆盖各自检查的边界。目录导入按文件提交，可以跳过不支持或无法读取的来源文件；已进入事务的失败必须向调用方报告，此前成功条目保留。

## 后台嵌入

导入和编辑完成后，Lua 使用通用持久调度发布 `embed-reindex`。参数仅保存 `quiet`、`background` 和十进制 `ticket`。插件私有 `knowledge:queue` 保存序号、等待标记和有序配置摘要；摘要包含规范化真实库路径、供应商、模型、分块、容量、端点及凭据条件，不保存明文凭据。

同一配置下，队列合并尚未取得 `knowledge:embedding` 锁的等待者，包括宿主已标记为 `running` 的进程。真正取得锁后才清除自身等待标记，之后的编辑可以再安排一项后续扫描；旧任务不能清除较晚序号。配置不同的任务不合并，相同相对目录在不同工作目录中也不会合并。

网络请求期间不持有数据锁。发布前重新检查当前元数据摘要和实际正文摘要，编辑或删除期间返回的旧向量不能写回。单文件新块在内存中分批构建，只发布最终完整快照；快照构造或发布失败保留旧语义索引。HTTP 单块失败沿用原行为：非静默模式报告失败，最终索引包含本次成功取得的块。

旧 `embedding.lock` 存在时继续阻止重建。新宿主锁在取消和进程退出后释放；其占用提示不要求删除旧锁文件。手动重建短暂尝试取锁，后台最多等待 600 秒，均受整个回调时限限制。

后台工作进程可在父命令退出后继续。新调度记录保存创建语言，启动、显式恢复和到期校验复用该语言，避免 CLI `--lang` 与环境语言不同而误判设置变化；实际源码、设置、启用状态和授权仍须重新验证。已开始的中断任务不自动重试，生命周期和取消边界见[持久调度接口](scheduler-api.md)。

## 资源边界

| 项目 | 限制 |
| --- | --- |
| 单个知识文件 | 默认 1 MiB，配置最多 4 MiB；上传头部也计入正文 |
| 每个索引镜像 | 默认且最多 8 MiB，配置最少 64 KiB |
| 恢复 JSON、单次输出 | 最多 4 MiB；JSON 转义和展示前缀也占额度 |
| 目录导入 | 每层最多 1024 项，共最多 4096 个普通文件，深度最多 64 |
| Lua 堆、共用二进制额度 | 分别最多 64 MiB |
| 共用计算 | 2000 万单位，Lua、摘要和 SQLite 等原生计算共同消耗 |
| 单次回调 | 最多 3600 秒、4096 次系统调用 |
| 嵌入 HTTP | 配置默认 60 秒，宿主最多 120 秒 |
| SQLite 请求或查询结果 | 最多 1 MiB，单个文字字段最多 256 KiB |

以上是独立上限，不保证任意组合都能完成。即使文件或数据库未达到容量上限，JSON 转义、向量维度、镜像复制、分块数量及累计计算仍可能先耗尽预算；超限不截断后宣称成功。实现按现有镜像和变更估算事务容量，避免小操作总是预留整个 8 MiB。

## 验证

冻结原版的 229 组纯规则与 307 组业务场景共 536 组对照覆盖分词、评分、片段、六工具、管理查询及正文变更；六项完整工具描述和 Schema 单独对照。仅归一上传时间，不修改预期掩盖业务差异。

正式文件宿主测试覆盖独立授权、计划模式、旧库结构、空库初始化、损坏和 WAL、恢复摘要冲突、跨实例取消、并发写入、HTTP 错误与凭据脱敏、向量批次发布及编辑删除竞争。容量回归包含完整 1 MiB 正文、40 个分块、64 条 1536 维向量，以及会触发缩页的宽字段。真实发布入口、全量回归和程序指纹见[迁移记录](migration.md)。
