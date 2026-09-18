# 第三方 Lua 扩展能力清单

本清单按外部包的实际调用契约组织。入口从[开发指南](getting-started.md)开始，最小组合见[项目笔记示例](../../examples/lua-plugins/project-notes/README.md)。使用普通包 ID 和显式授权即可调用“已公开”能力，无需增加内置注册或重新编译宿主。

“已公开”表示实现及公共契约已经存在，不代表所有平台或每种外部组合都完成实际运行验证。下表以当前 v1 源码及对应验证构建为依据，不保证所有历史 v1 程序都包含新增接口。插件在发布说明中记录实际验证的 sai 构建。

## 统一约束

有效权限为清单声明与用户授权的交集。模型和工具组合还受当前 Agent 的工具白名单与实时权限模式约束。外部包安装后默认禁用；普通 `enable` 不添加权限，替换更新不自动授予新增能力。

下表中的“回调”指工具和用户命令。初始化只能读取自身设置、平台元数据、纯计算及同步注册；宿主 I/O 只在有效调用期间开放。事件默认只读且没有模型或工具服务。每项请求还受对应的字节、次数、时长和累计计算预算限制，修改 Lua 中的上下文字段不会扩大权限。

## 已公开：注册、上下文与组合

| 第三方场景 | 接口与权限 | 调用阶段、适用入口和限制 | 公共契约 |
| --- | --- | --- | --- |
| 向模型提供新工具 | `sai.register_tool`；注册本身无外部能力要求 | 初始化注册；CLI 对话、TUI、Web、子任务工具表。外部名称为 `lua__<id>__<name>`；`read_only`、`writes`、`optional_writes` 明确区分 | [工具注册](api.md#注册工具) |
| 提供用户主动执行的命令 | `sai.register_command` | 初始化注册；`sai plugins run` 与 TUI `/plugin`。Web 和子任务没有直接命令界面；计划模式拒绝写入命令 | [用户命令](api.md#注册用户命令) |
| 观察生命周期、收窄工具许可 | `sai.on`；无额外事件授权 | Agent、模型请求、工具事件使用有界数据；`tool_call` 只能拒绝。普通事件不能修改真实参数或消息、调用工具或请求模型 | [生命周期](api.md#生命周期) |
| 读取调用上下文、报告进度 | `ctx.session_id/operation_id/workdir/allow_writes`、`ctx.progress`、`ctx.json_integer` | 宿主提供会话与任务事实，不提供整段对话；进度限 4096 字节 × 128 条；整数查询只在工具和事件中提供 | [上下文](api.md#上下文与状态) |
| 使用当前模型完成一次请求 | `sai.model.complete`；`model`，CLI `--allow-model` | 工具/命令回调；沿用当前 Agent 或子任务客户端。插件自行构造消息，不隐式加入主会话；不能指定或读取供应商凭据 | [模型与工具](api.md#模型与工具) |
| 组合原生工具及其他 Lua 工具 | `sai.tools.list/call`；精确 `tools`，CLI `--allow-tool` | 回调；不绕过 Agent 白名单、工具审计或实时只读限制；排除本插件及祖先，最多 8 层；调用返回文本，JSON 由插件显式解码 | [模型与工具](api.md#模型与工具) |
| 给回复增加有界上下文或完成动作 | `sai.register_reply_policy`；`reply_policy`，CLI `--allow-reply-policy` | Sai 内核主回复准备时收到当前用户消息，可使用另行授权的模型与只读工具；完成阶段不提供主回复正文。`after_tool` 仅普通串行分支且无模型/工具服务；外部内核不执行这些策略 | [回复策略](reply-policy-api.md) |
| 计算交互面的答复通知 | `sai.on("reply_end", ...)`；`notifications`，CLI `--allow-notifications` | TUI/Web 的独立纯 VM；没有正文、会话存储或 I/O，不与 Agent 实例共享全局变量；CLI 单次调用及子任务不提供此展示入口 | [通知纯回调](api.md#通知纯回调) |
| 配置 TUI 底栏 | `sai.on("tui_status", ...)`；`tui_status`，CLI `--allow-tui-status` | 独立后台纯 VM；仅接收显示状态，返回有界左右文本，失败回退默认底栏 | [底栏接口](tui-status-api.md) |

## 已公开：系统服务与持久状态

| 第三方场景 | 接口与权限 | 调用阶段、适用入口和限制 | 公共契约 |
| --- | --- | --- | --- |
| 访问已知 HTTP 服务 | `sai.http.request`；精确 `http` 来源，必要时 `http_read_only_post` | 回调及只读事件；GET/HEAD 可只读，查询 POST 需精确端点，其余需可信写入许可；重定向继续授权 | [HTTP](http-api.md) |
| 读取用户给定的任意 URL | `sai.http.request`、`sai.binary.request`；`http_read_any`，CLI `--allow-http-read-any` | 只开放 GET/HEAD，包含本地服务；初始请求可提供认证头，跨来源跳转移除敏感头。不继承给匿名下载，也不授权写入方法 | [任意来源与响应选项](http-api.md) |
| 操作原始响应、二进制与图片 | `sai.binary.request/decode_base64/from_bytes/read_file` 及缓冲方法 | 请求复用 HTTP 授权；本地读取复用 `system.read_paths`；句柄限定当前回调，另有二进制额度。写入与展示另行授权 | [二进制接口](binary-api.md) |
| 匿名下载公开文件 | `sai.binary.download`；精确 `http` 来源或 `binary.public_downloads`，后者通过 `--allow-public-downloads` 授权 | 精确授权来源可以是本地服务；其他来源每次 DNS/重定向均须通过公网检查。不能带自定义认证头，不继承 `http_read_any` | [下载接口](binary-api.md) |
| 从较大网络或文件正文提取有限文本 | `buffer:document`；输入来源沿用网络或文件授权 | 当前回调内纯内存转换，可只读；原文、HTML 纯文本或 Markdown，最多 8 MiB 输入，全文字符计数、JSON 输出及共享原生预算均受检查 | [正文转换](document-api.md) |
| 读取文本、目录和文件元数据 | `sai.fs.read_text/read_dir/stat/realpath`；`system.read_paths`，CLI `--allow-read-path` | 回调及只读事件；路径相对于可信任务目录；读取结果显式标记截断，越界链接与特殊文件拒绝 | [系统接口](system-api.md#文件与目录) |
| 创建目录、发布普通文件 | `sai.fs.create_dir`、`buffer:write`、`buffer:write_if`；`binary.write_paths`，CLI `--allow-write-path` | 需写入回调与可信写入许可；条件发布另需读取授权，不提供多文件事务 | [文件发布](binary-api.md#文件修订与条件写入) |
| 删除文件、移入回收站 | `sai.fs.remove_file/trash_file`；各自 `system.remove_paths/trash_paths` | 写入回调；独立授权，不由读取/写入自动推导。只接受普通文件；回收站当前仅 Linux 同文件系统 | [删除接口](file-removal-api.md) |
| 读取平台及选定环境变量 | `sai.system.platform/process_id`、`sai.env.get`；环境需 `system.environment` | 平台元数据加载期可用；环境读取限回调及事件，必须精确授权名称，不等同于读取全部宿主环境 | [平台与环境](system-api.md#平台与环境) |
| 执行固定系统命令 | `sai.process.output`；完整 `system.processes` 模板，CLI `--allow-process` | 固定程序和参数占位符；只读事件仅允许已授权只读模板。模板变化需要重授；Unix 进程组、Windows Job 回收 | [模板进程](system-api.md#模板进程) |
| 保存本插件的会话状态 | `sai.storage.get/set/compare_exchange`；`system.session_storage` | 工具/命令可更新私有会话记录，包括只读工具；事件只能读取。工具与独立命令使用不同会话作用域 | [会话记录](private-api.md#会话记录) |
| 保存跨会话偏好或检查记录 | `sai.storage.plugin.get/set/compare_exchange`；`system.plugin_storage` | 读取可只读；写入/删除需可信写入许可。独立于会话存储；禁用与卸载保留记录 | [插件持久记录](private-api.md#插件持久记录) |
| 协调同一插件的多个实例 | `sai.storage.plugin.with_lock`；同一 `system.plugin_storage` | 回调及只读事件；锁不授予写入权限。最多四层递增键，等待及取消有界，不能作为跨插件共享存储 | [作用域锁](private-lock-api.md) |
| 创建临时工作区、展开源码包 | `sai.workspace.open` 与目录句柄；`system.workspace` | 回调；会话私有缓存、路径受限、归档大小受限。下载另需精确 HTTP 来源或 `http_read_any`，进程另需 `workspace=true` 完整模板授权 | [工作目录与归档](private-api.md#工作目录) |
| 查询或变更 SQLite 快照 | `sai.sqlite.query/apply` | 当前回调内的纯内存镜像，可只读计算；文件读取/发布仍需目录授权。结构化请求，不接受任意 SQL 或路径 | [SQLite 接口](sqlite-api.md) |
| 延迟执行本插件命令 | `sai.scheduler.schedule/list/get/cancel/resume`；`system.schedule` | 查询可只读，创建/取消/恢复需写入许可；宿主独立进程执行，到期复核版本、设置与授权。不接受任意宿主命令 | [持久调度](scheduler-api.md) |
| 立即投递桌面通知或声音 | `sai.notify.send`；`system.notify`，CLI `--allow-notify` | 写入回调；本地声音另需文件读取权限。发送到宿主设备，不自动投递给 Web 浏览器或远端客户端 | [主动通知](notification-api.md) |
| 分析或在终端展示图片 | `sai.vision.info`、`buffer:analyze_image`；`vision`；`sai.terminal.size/display_image`，展示需 `binary.display_images` | 视觉使用独立宿主视觉模型；终端显示取决于实际终端，Web 不能借此注册界面组件；各入口均受图片和输出预算约束 | [视觉和终端](binary-api.md) |
| JSON、文本、字节编码、摘要和时间组合 | `sai.json`、`sai.text`、`sai.encoding`、`sai.crypto`、`sai.time` | 无外部授权；纯计算可在初始化及回调使用，仍受输入输出与累计预算约束；大整数使用原始 JSON 整数查询 | [纯计算接口](api.md#json文本与时间) |

`private-api.md` 中的“私有”指数据归属当前插件；上述存储、锁和工作目录均是第三方公共 API，并非内置专用入口。

## 已公开：开发与本地分发

| 第三方场景 | 公共入口 | 当前行为与限制 | 说明 |
| --- | --- | --- | --- |
| 创建、检查独立源码包 | `plugins init/check` | 无需模型配置；检查受限初始化与注册，不授予能力或执行全部业务分支 | [开发指南](getting-started.md) |
| 生成可独立发布的源码归档 | `plugins pack`、`--output`、`--json` | 仅清单与 Lua；标准 tar.gz、固定元数据和 SHA-256；不覆盖已有输出，接收者先解压 | [打包与分发](distribution.md) |
| 安装、配置、更新与移除 | `plugins install/configure/enable/disable/remove` | 新包默认禁用；`install --replace` 保留设置和授权，不自动扩权；卸载保留持久记录 | [维护路径](distribution.md#替换撤权与卸载) |
| 检查加载状态与刷新会话 | `plugins info/list/commands/run`、TUI `/plugins reload` | `info` 不显示设置秘密；CLI 下次调用读取新配置，已开始的调用保留原快照 | [加载与重载](getting-started.md#在-tui-会话中使用) |

## 业务示例与可选适配

| 旧耦合 | 当前处理 | 外部包使用方式 |
| --- | --- | --- |
| 主配置业务字段与凭据投影 | 已删除，保存设置不派生权限 | 自己的 `plugins.jsonc` 设置；环境变量逐项声明并授权 |
| 旧短工具名与保留包 ID | 已删除，所有包使用普通安装来源 | `lua__<id>__<name>`；安装不会修改 Agent 白名单 |
| 闹钟旧任务 | 仅显式 CLI 管理可以查询、取消；禁止重放 | 公共调度只处理本插件的新任务，不因 `alarm` ID 取得旧记录 |
| 待办旧记录与可选视图 | 显式导入到公共会话存储，未安装时返回空视图 | 有界文件读取或明确传入完整状态；不自动读取旧会话文件 |
| 知识库文件与目录导入 | CLI 与 TUI 调用同一普通命令，不临时扩权 | `plugins run`、清单与用户授权；`add-text` 可接收明确传入的正文 |

具体迁移方法见[旧版迁移说明](bundled-plugins.md)。全部 25 个业务包的清单和源码见[示例索引](../../examples/lua-plugins/README.md)，核心保留公共宿主机制。

## 尚未公开与暂不开放

| 状态 | 场景或能力 | 当前边界及处理方式 |
| --- | --- | --- |
| 尚未公开 | 从任意回调按需查询主会话消息历史或会话列表 | `ctx` 与事件只提供规定的有界字段；回复策略 `prepare` 已能收到当前轮用户消息，但没有通用历史查询接口，模型请求也不自动带上主对话 |
| 尚未公开 | 自定义 Web 面板、TUI 小组件或通用弹窗 | 已开放底栏纯文本布局，尚无任意小组件或弹窗接口；不能把终端图片等同于跨端组件 API |
| 尚未公开 | 主动切换模型、会话或修改主 Agent 历史 | 当前模型接口只使用宿主选定客户端；回复策略可提供隔离的上下文，不改写历史事实 |
| 尚未公开 | 宿主版本范围、通用功能查询、稳定机器错误码 | 当前仅有 `api_version` 精确校验、插件 SemVer、Lua/宿主错误文本；需记录实际测试构建并明确依赖 |
| 尚未公开 | 自动远程安装、插件依赖求解与签名校验 | 已有本地 `pack` 与源码目录分发，压缩包先解压；安装、替换更新、撤权与移除已有公共入口 |
| 暂不开放 | 整份主配置、供应商密钥、任意环境枚举 | 供应商凭据留在宿主；自身设置和精确环境授权不能扩大到主配置读取 |
| 暂不开放 | 通过事件批准宿主拒绝的操作、提高资源上限 | 事件只能观察或收窄；权限、会话归属、预算和回收由 Rust 决定 |
| 暂不开放 | 原生动态库、无约束 `io/os/debug`、跨包任意文件访问 | 使用已有授权文件、进程和网络接口；沙箱不暴露绕过路径 |

## 版本与开发支持

[兼容与维护规则](compatibility.md)说明 v1 增量接口、弃用、错误语义、更新不扩权及数据回退。`api_version=1` 不表示旧构建认识后来增加的清单字段；例如 `http_read_any` 需要支持该字段的宿主，不能靠插件自身版本声明兼容。

[编辑器支持](editor-support.md)提供模板、项目笔记和 URL 预览所用接口的 LuaLS 声明，包含参数、返回值及正文格式枚举。覆盖范围按文件列明，不代表全部 API 已有类型声明；静态提示不能代替权限和运行时校验。

## 核对实现与验证范围

注册、参数和资源约束来自运行时 [manifest.rs](../../crates/sai-plugin-runtime/src/manifest.rs)、[capabilities.rs](../../crates/sai-plugin-runtime/src/capabilities.rs) 及 [runtime/](../../crates/sai-plugin-runtime/src/runtime/)；正式宿主适配与安装管理位于 [src/plugins/](../../src/plugins/)。

外部开发路径的代表性验收由 [external_workflow.rs](../../src/plugins/tests/external_workflow.rs) 和 [CLI 验收脚本](../../scripts/plugin-smoke/verify.py) 覆盖：普通安装身份、最小权限、输入与资源边界、跨进程记录、更新不扩权、重载、撤权和卸载。它们证明该组合可以独立开发，不替代模型、HTTP、进程、调度及其他接口的已有契约测试，也不证明未执行的平台。

[生命周期集成契约](../../src/plugins/tests/lifecycle_workflow.rs)以只注册命令和监听器的普通安装包验证真实主请求与子任务执行器：工具过滤保留监听器，事件保持只读，原始工具参数不能被监听器改写，同一操作的标识一致；流式取消不补发结束事件，下一次请求仍能使用该实例。这些测试使用本地模型协议服务，交互面本身仍需各自验证。

任意 URL 和正文场景使用[普通外部宿主契约](../../src/plugins/tests/http_public_contract.rs)、[归档授权契约](../../src/plugins/tests/http_archive.rs)及 [URL 预览 CLI 验收](../../scripts/plugin-smoke/verify_http.py)。运行时另检查参数、预算、取消和句柄期限；CLI 仅使用本地 HTTP 服务与隔离 XDG 配置，不需要模型或外网。

源码分发通过[打包测试](../../src/plugins/tests/package_archive.rs)、[清单保存边界测试](../../crates/sai-plugin-runtime/tests/package_serialization.rs)和[分发 CLI 验收](../../scripts/plugin-smoke/verify_distribution.py)验证。覆盖固定内容与元数据、不覆盖和并发发布、解压后执行及替换更新，另核对接近 64 KiB 的清单不会在安装保存后变成不可加载文件。
