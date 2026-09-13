# 核心能力与示例插件边界

更新日期：2026-09-14。原有 25 个 Lua 业务包均已提取为普通外部示例，无示例源码的发布构建与无插件核心验收通过；完整证据见[归档状态](../../.doc/archive/2026-09-14-core-plugin-extraction/status.md)。

## 核心保留范围

核心保留请求执行、可信会话事实、权限审计、预算与取消、公共宿主服务，以及插件安装和运行机制。原有 25 个 Lua 包均提供可拆卸业务功能；没有必须常驻的核心 Lua 包，原 `plugins/` 目录已经删除。

| 核心职责 | 实现边界 | 业务包卸载后的要求 |
| --- | --- | --- |
| 请求与会话 | Agent、runner、会话记录、工具派发 | 基础对话、文件工具和会话恢复继续工作 |
| 信任与资源控制 | `crates/sai-plugin-runtime/`、权限和取消机制 | 授权不因安装、更新或卸载而扩大 |
| 插件管理 | `src/plugins/` 的发现、安装、配置、注册、命令与生命周期 | 没有任何示例时可以正常构建、启动和列举空集合 |
| 公共宿主 | HTTP、文件、进程、存储、模型、通知与调度接口 | 不按业务包 ID 注入凭据、授权或专用身份 |
| 通用展示与事件 | 会话事实、工具事件、可选视图和回复策略执行 | 缺少业务提供方时返回空视图或明确的功能不可用信息 |

待办条目、知识库文档和闹钟规则是业务数据；承载它们的会话记录、文件、任务持久化和取消机制是公共能力。已有专用调用并不能证明整个业务包属于核心。

## 逐包归属

下表全部归属为 `examples/lua-plugins/` 中的普通安装包，四批安装、授权、更新、撤权和卸载验收均已通过。各包保留自身业务实现、输入校验和有效回归样本；包名链接提供配置、权限、调用入口、依赖和数据保留说明。

| 包 | 归属依据 | 依赖与独立化处理 | 实施批次 |
| --- | --- | --- | --- |
| [hash-codec](../../examples/lua-plugins/hash-codec/README.md) | 摘要和文本解码，不参与核心执行 | 无外部能力；移除旧开关和短工具名，验证普通身份 | 1 |
| [weather](../../examples/lua-plugins/weather/README.md) | 特定天气服务查询 | 仅显式授予 `https://wttr.in`；移除旧开关和默认工具 | 1 |
| [archlinux](../../examples/lua-plugins/archlinux/README.md) | Arch/AUR/ArchWiki 查询 | 清单中的四个精确 HTTP 来源；处理旧开关和调用方名称 | 2 |
| [deepseek-status](../../examples/lua-plugins/deepseek-status/README.md) | 服务状态展示 | 精确 HTTP 来源；取消自动启用 | 2 |
| [exchange-rate](../../examples/lua-plugins/exchange-rate/README.md) | 汇率服务路由 | 独立 API 密钥与免费回退设置；不再投影主配置 | 2 |
| [fcitx-wiki](../../examples/lua-plugins/fcitx-wiki/README.md) | 输入法文档检索 | 精确 HTTP 来源；调查包使用公开工具名称 | 2 |
| [linux-game-signals](../../examples/lua-plugins/linux-game-signals/README.md) | 游戏兼容性证据采集 | 四个 HTTP 来源；更新调查包依赖 | 2 |
| [moegirl](../../examples/lua-plugins/moegirl/README.md) | 百科查询 | 显式声明重定向来源；删除旧开关 | 2 |
| [online-man](../../examples/lua-plugins/online-man/README.md) | 在线手册检索 | 两个手册来源；更新默认工具和调用方 | 2 |
| [protondb](../../examples/lua-plugins/protondb/README.md) | 游戏兼容性查询 | ProtonDB 与 Algolia 来源；调查包使用普通工具名称 | 2 |
| [web-fetch](../../examples/lua-plugins/web-fetch/README.md) | URL 正文读取策略 | 任意来源只读 HTTP 单独授权；保留字节与输出边界 | 2 |
| [web-search](../../examples/lua-plugins/web-search/README.md) | 多供应商搜索策略 | 独立地址、凭据与查询 POST 声明；清理主配置投影 | 2 |
| [xuanxue](../../examples/lua-plugins/xuanxue/README.md) | 抽取和骰子业务 | 无外部能力；保留输入与概率边界 | 2 |
| [diagnostic-evidence](../../examples/lua-plugins/diagnostic-evidence/README.md) | 系统诊断业务规则 | 文件、环境、进程模板和 `run_command` 显式授权；独立限制设置 | 3 |
| [package-advisor](../../examples/lua-plugins/package-advisor/README.md) | AUR 审查和安装工作流 | 公共会话存储、私有工作目录、进程模板；继续逐次确认写入 | 3 |
| [input-method-investigation](../../examples/lua-plugins/input-method-investigation/README.md) | 输入法调查编排 | 模型和多个可选检索工具；独立设置、缺失依赖诊断和名称迁移 | 3 |
| [linux-game-investigation](../../examples/lua-plugins/linux-game-investigation/README.md) | 游戏调查编排 | 证据采集、ProtonDB、搜索与模型；显式工具授权 | 3 |
| [image-display](../../examples/lua-plugins/image-display/README.md) | 终端图片展示策略 | 公共图片展示能力留在宿主；尺寸设置归插件 | 3 |
| [image-generation](../../examples/lua-plugins/image-generation/README.md) | 图片生成与保存工作流 | 独立供应商、输出路径和展示依赖；HTTP、下载和写入分别授权 | 3 |
| [web-images](../../examples/lua-plugins/web-images/README.md) | 图片搜索与筛选工作流 | 独立目录、视觉配置和展示依赖；缺少展示包不影响查询 | 3 |
| [alarm](../../examples/lua-plugins/alarm/README.md) | 提醒规则与声音选择 | 公共调度和通知留在宿主；显式音频授权，旧任务仅管理和取消 | 4 |
| [reply-notification](../../examples/lua-plugins/reply-notification/README.md) | 回答完成后的通知策略 | 主机保留投递机制；设置归插件，无策略时不自动通知 | 4 |
| [memes](../../examples/lua-plugins/memes/README.md) | 图库管理与自动发送规则 | 独立图库和索引路径、回复策略、视觉授权；显式数据接续 | 4 |
| [todo](../../examples/lua-plugins/todo/README.md) | 待办维护与回复前检查 | 公共会话存储与显式导入，缺失、禁用或存储撤权时视图为空 | 4 |
| [knowledge-base](../../examples/lua-plugins/knowledge-base/README.md) | 文档检索、索引与嵌入任务 | 独立数据与供应商配置；CLI/界面改为可选外部调用，不隐式授权 | 4 |

## 已解除的宿主耦合

- 构建不再跟踪或嵌入业务包：删除原 `bundled.rs` 和业务兼容目录，[build.rs](../../build.rs) 只处理核心资源。
- [discovery.rs](../../src/plugins/discovery.rs) 只发现用户安装目录，授权缺省为空；[registry.rs](../../src/plugins/registry.rs) 只注册当前启用包，不补回禁用的业务工具。
- [agent_presets.rs](../../src/config/agent_presets.rs) 和 [plugins.rs](../../src/config_tui/plugins.rs) 已移除业务默认工具及设置菜单；Web 旧业务设置页同步移除，安装不修改 Agent 白名单。
- [private/host.rs](../../src/plugins/private/host.rs) 只绑定普通插件标识和修订；[todo_view.rs](../../src/plugins/todo_view.rs) 通过公共命令读取计划，不按 ID 访问旧文件。清空或删除会话调用通用存储清理，跨工作区会话及持久记录隔离。
- [commands.rs](../../src/plugins/commands.rs) 和 [knowledge_view.rs](../../src/plugins/knowledge_view.rs) 调用普通安装包。TUI 文件及目录导入遵守来源读取授权；缺失插件时列表为空，修改入口报告不可用。
- [scheduler](../../src/plugins/scheduler/) 仅处理通用任务；[旧闹钟管理](../../src/cli/plugin_jobs.rs) 显式接入旧记录，保留稳定进程身份核验、取消和禁止重放。兼容入口不恢复音频许可。
- 图库图片及索引移入 [memes/assets](../../examples/lua-plugins/memes/assets/)，不随程序或 Arch 包交付。用户显式复制基础图库；Lua 插件归档仍只含清单和源码。

## 迁移约定

示例工具统一命名为 `lua__<插件 ID>__<包内工具名>`。旧短名称不映射到任意同名外部包，也不自动追加 Agent 白名单。用户安装并显式启用后，再按用途选入相应 Agent。

旧主配置中的开关不安装示例，不恢复已撤销权限。业务凭据和数据不自动复制进示例源码。需要自定义地址或目录时，同时调整插件设置、清单声明和用户授权；单独保存设置不能扩大权限。

卸载默认保留插件业务数据和会话历史。普通安装、更新、禁用、撤权和移除仍经过公共管理流程；数据导入与删除分别说明，不混入源码安装步骤。

待办使用公共会话存储：卸载保留计划，清空对话会清除当前计划；整体清空或删除会话还会删除目录内的旧快照，目录外备份保留。知识库原文和索引、图库及发送记录、调度历史均不因卸载而删除。各类数据迁移见[旧版迁移说明](bundled-plugins.md)。

## 证据状态

`hash-codec` 与 `weather` 已移入示例目录，31 项定向测试和 49 次真实 CLI 调用通过。编码保留 175 组冻结结果；天气的实际 HTTP 验收使用仓库外本地服务派生包，原版服务契约由固定宿主验证。证据见 `target/plugin-extraction-batch-1/`。

第二批 11 个查询与内容包已提取，相关 Rust 用例最终 214 项通过，Web 类型检查与 13 项用例通过；193 次真实 CLI 调用验证普通管理流程及代表性业务。原站点协议使用固定样本，自建搜索 CLI 使用仓库外派生清单。证据见 `target/plugin-extraction-batch-2/`。

第三批 7 个诊断、调查、图片与软件包工作流已经提取，259 项相关 Rust 用例及两个直接调用契约通过；124 次 CLI 调用验证普通管理与本地图片业务，修复了 Plan 模式选择可选写入工具的路径。证据见 `target/plugin-extraction-batch-3/`。

第四批 5 个数据与策略包已提取，163 次 CLI 调用通过，覆盖实际会话导入、数据保留、知识库目录与原文操作、图库管理、纯通知预览、未来闹钟创建后立即取消及旧任务拒绝重放。应用回归累计 3,076 项通过，13 项既有忽略；Web 类型检查、设置用例和构建通过。证据见 `target/plugin-extraction-batch-4/`。

开发程序的无插件验收已通过：15 次 CLI、24 次 Web API 和独立伪终端交互覆盖文件工具、计划模式拒绝写入、模型工具回合、会话管理、空待办/知识库、流式取消及继续。7 次模型请求均无 Lua 工具，其中包含正常的会话标题生成。

全部示例源码移出仓库后，发布构建通过，耗时 146.723 秒；发布程序在源码继续缺席时通过相同核心验收，Arch 包仅包含程序和包元数据，示例源码随后恢复。发布程序又通过全部 27 个示例的包检查。程序 SHA-256 为 `1c525f878a6431956d812ec183d9936eb9dac1e23533f4dd7719a0378b52ce39`，证据见 `target/plugin-extraction-batch-4/{release-report,release-core-report,release-examples-report}.json`。

验证使用隔离路径、本地协议服务和确定性样本；没有访问真实模型供应商、向真实桌面投递通知或播放声音，也不代表已验证所有外部站点及其他操作系统。
