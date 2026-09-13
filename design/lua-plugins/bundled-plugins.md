# 原内置插件迁移说明

原有 25 个 Lua 业务包已移到 [examples/lua-plugins](../../examples/lua-plugins/README.md)。宿主不再内嵌、默认安装或默认授权这些包；全部通过普通插件管理入口安装、配置、更新、禁用和卸载。最终分类见[核心与示例边界](plugin-boundaries.md)。

## 安装与工具名称

```sh
sai plugins check ./examples/lua-plugins/weather
sai plugins pack ./examples/lua-plugins/weather --output ./weather.tar.gz
sai plugins install ./examples/lua-plugins/weather
sai plugins configure weather ./examples/lua-plugins/weather/settings.example.json
sai plugins enable weather --allow-http https://wttr.in
sai --plan plugins call weather get_weather '{"location":"Beijing"}'
```

所有安装包默认禁用、零授权。模型可见名称统一为 `lua__<插件 ID>__<包内名称>`，例如 `get_weather` 改为 `lua__weather__get_weather`，`todo` 改为 `lua__todo__todo`。包内名称、参数 Schema 与正常业务结果沿用各自契约；`fcitx5_input_method_wiki_qurey` 保留原拼写。

默认 Agent 不再包含这些业务工具，安装也不会修改 Agent 白名单。需要在对应 Agent 中明确选择完整名称，再创建会话或执行 `/plugins reload`。组合插件按各自 README 声明必需和可选依赖；缺少显示插件时仍保留已经保存的图片。

`plugins call` 无需模型，接受包内名或该包的完整名称；`plugins run` 执行注册命令。二者可在插件 ID 前使用 `--session <ID>` 选择当前工作区的真实会话。省略时使用按工作区隔离的命令作用域。写入操作仍要求允许写入的模式，工具调用仍接受 Schema、权限、预算与取消检查。

## 配置与授权

旧主配置中的业务开关、`notification` 设置、目录及供应商字段不再投影到示例。每个包在 `plugins.jsonc` 中保存独立设置，仓库提供 `settings.example.json`。配置命令以文件内容替换该包设置，不复制宿主密钥或用户数据。

| 设置类型 | 当前处理 |
| --- | --- |
| 开关和语言 | `plugins enable/disable` 独立控制；语言按插件设置校验 |
| HTTP 地址 | 设置决定请求地址；清单声明与用户授权共同限制可访问范围 |
| API 密钥 | 插件显式字符串，或其支持且已经授权的环境变量；不继承宿主供应商密钥 |
| 输入与输出目录 | 设置决定业务路径；清单和授权必须同时覆盖，保存设置不扩权 |
| 工具依赖 | 使用完整外部工具名，分别满足包声明、用户授权及 Agent 白名单 |
| 通知与回复策略 | 投递、模型、视觉和策略分别授权；未安装策略时没有业务投递 |

`--grant-declared` 会明确授予当前清单的全部能力。各包 README 优先列出按用途选择的最小授权；同类目录选项替换该项集合，应列出仍需使用的全部目录。替换安装保留既有授权，不自动授予新能力。普通 `enable` 不恢复撤销项。

## 数据与可选入口

| 包 | 接续方式 | 未安装、禁用或卸载后的行为 |
| --- | --- | --- |
| [todo](../../examples/lua-plugins/todo/README.md) | 使用 `import` 显式导入旧完整快照或组装后的记录；保存到公共会话存储 | 可选视图为空；卸载保留记录，清空对话或删除会话遵守公共存储清理规则 |
| [knowledge-base](../../examples/lua-plugins/knowledge-base/README.md) | 独立设置原数据目录并声明、授权；嵌入供应商独立配置 | 可选列表为空，修改入口报告不可用；知识文件与两个数据库保留 |
| [memes](../../examples/lua-plugins/memes/README.md) | 显式设置基础图库、用户库和发送状态目录；示例图片单独复制 | 不再提供图库工具或自动发送；已有图片、索引和发送记录保留 |
| [alarm](../../examples/lua-plugins/alarm/README.md) | 新任务使用公共调度；旧记录仅通过显式管理入口查询和取消 | 不创建新提醒；已有任务历史保留，旧任务禁止重放 |
| [reply-notification](../../examples/lua-plugins/reply-notification/README.md) | 将通知和声音偏好写入插件设置 | TUI、Web 不执行此通知策略；基础答复继续工作 |

待办不再自动读取 `todos.json`、`todos.history.json` 或 `todos.plugin.json`；旧文件保持原样，导入拒绝覆盖非空当前计划。知识库 CLI 与 TUI 的文件、目录导入遵守普通读取授权；独立 `add-text` 命令保存明确提供的原始正文。图库不再查找 `src/memes` 或 `/usr/share/sai/memes`，宿主安装包不包含图库图片。

旧闹钟管理使用 `sai plugins jobs alarm list/cancel/resume`。公共 Lua 调度接口只处理新任务；旧任务记录保持原文件不变，管理取消核对稳定进程身份，`resume` 明确拒绝旧记录。保留的旧工作入口执行当前普通安装包，仍需完整授权，不借用历史音频许可。

## 更新与移除

```sh
sai plugins install ./new-version --replace
sai plugins enable PLUGIN_ID --no-http
sai plugins disable PLUGIN_ID
sai plugins remove PLUGIN_ID
```

卸载删除源码、禁用配置并撤销授权，默认保留业务数据。现有调用使用原快照到完成或取消，新调用与后台到期检查使用当前启用和授权状态。重新安装仍需明确启用、授权；源码回退不会回退数据。

每个包的设置、命令、平台支持和数据限制以[示例索引](../../examples/lua-plugins/README.md)链接的 README 为准。[历史迁移记录](migration.md)保留此前内置阶段的实现与验收事实，不作为当前默认授权或配置说明。
