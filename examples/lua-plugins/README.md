# Lua 示例插件

这里的包使用普通外部安装身份。源码不会随 sai 自动内嵌，安装后默认禁用且没有授权；启用、配置、更新和卸载均使用公共插件管理入口。

| 示例 | 用途 | 最小能力 |
| --- | --- | --- |
| [tui-status](tui-status/README.md) | 配置 TUI 底栏字段、顺序与窄窗口显示 | `tui_status` 纯展示 |
| [hash-codec](hash-codec/README.md) | 摘要与文本解码 | 无外部能力 |
| [weather](weather/README.md) | 当前天气查询 | `https://wttr.in` 的 HTTP 读取 |
| [archlinux](archlinux/README.md) | AUR、官方软件包、Wiki 与状态 | 对应 Arch 来源的 HTTP 读取 |
| [deepseek-status](deepseek-status/README.md) | 官方服务状态 | 状态页 HTTP 读取 |
| [exchange-rate](exchange-rate/README.md) | 汇率查询 | 免费或配置来源的 HTTP 读取 |
| [fcitx-wiki](fcitx-wiki/README.md) | 输入法规则与文档 | 包内规则无需权限；摘录需 HTTP |
| [linux-game-signals](linux-game-signals/README.md) | 游戏兼容性证据采集 | 四个资料来源的 HTTP 读取 |
| [moegirl](moegirl/README.md) | 百科搜索与正文 | 所选站点的 HTTP 读取 |
| [online-man](online-man/README.md) | 在线 Linux 手册 | Arch 手册与 man7 HTTP 读取 |
| [protondb](protondb/README.md) | 游戏评级与评论 | ProtonDB 与查询来源 HTTP |
| [web-fetch](web-fetch/README.md) | 已知 URL 正文 | 任意来源只读 HTTP |
| [web-search](web-search/README.md) | 多供应商搜索 | 所选供应商 HTTP、查询 POST 和可选环境变量 |
| [xuanxue](xuanxue/README.md) | 抽取与骰子 | 无外部能力 |
| [diagnostic-evidence](diagnostic-evidence/README.md) | 系统诊断证据 | 按领域选择文件、环境和进程模板 |
| [input-method-investigation](input-method-investigation/README.md) | 输入法调查 | 模型和所选资料工具 |
| [linux-game-investigation](linux-game-investigation/README.md) | 游戏兼容性调查 | 模型及必需的游戏信号工具 |
| [package-advisor](package-advisor/README.md) | AUR 审查与安装 | HTTP、隔离工作目录、会话存储和进程模板 |
| [image-display](image-display/README.md) | 终端图片显示 | 图片展示 |
| [image-generation](image-generation/README.md) | 图片生成与保存 | API 来源、写入目录，可选下载和显示工具 |
| [web-images](web-images/README.md) | 图片查询与筛选 | 查询来源；下载、视觉和预览分别授权 |
| [alarm](alarm/README.md) | 提醒与声音投递 | 持久调度；投递另需通知和可选音频读取 |
| [reply-notification](reply-notification/README.md) | TUI 与 Web 答复通知策略 | 预览无需权限；正式通知单独授权 |
| [memes](memes/README.md) | 图库与可选自动发送 | 专用目录读取；写入、显示、视觉和策略分别授权 |
| [todo](todo/README.md) | 会话清单、历史与工具循环提醒 | 会话存储；提醒另需回复策略 |
| [knowledge-base](knowledge-base/README.md) | 本地知识文件、搜索与嵌入 | 库目录与插件存储；嵌入另需 HTTP 和调度 |
| [project-notes](project-notes/README.md) | 文件读取与持久记录 | 笔记目录读取；保存记录另需插件存储 |
| [url-preview](url-preview/README.md) | URL 正文预览 | 任意来源只读 HTTP |

原有 25 个业务包全部位于本目录，另保留两个开发入门示例。核心保留公共执行、授权、存储、通知和调度机制，分类依据见[边界清单](../../design/lua-plugins/plugin-boundaries.md)。

## 通用使用路径

从各包目录检查、打包，再安装源码或解压后的归档：

```sh
sai plugins check ./path/to/plugin
sai plugins pack ./path/to/plugin --output ./plugin.tar.gz
sai plugins install ./path/to/plugin
sai plugins configure PLUGIN_ID ./settings.json
sai plugins enable PLUGIN_ID
sai plugins info PLUGIN_ID --json
sai --plan plugins call PLUGIN_ID TOOL_NAME '{"argument":"value"}'
```

`enable` 本身不授予外部能力。各包 README 列出实际授权选项；只有需要写入的操作才使用允许写入的模式。`plugins call` 接受包内工具名和 JSON 参数，也接受该包自己的完整工具名，沿用工具 Schema、权限、审计与资源预算，不需要模型。注册了用户命令的包也可使用 `plugins run`。

`plugins call` 和 `plugins run` 可在插件 ID 前指定 `--session <会话 ID>`，选择当前工作区的真实会话存储。省略时使用按工作区隔离的命令作用域；它不会自动修改交互会话中的待办或其他插件记录。

模型使用的工具名称为 `lua__<插件 ID>__<工具名>`。安装不会自动修改 Agent 白名单；在对应 Agent 中选入所需工具，再创建会话或重新加载。旧主配置开关和旧短工具名不会自动安装或授权示例。

```sh
sai plugins install ./path/to/new-version --replace
sai plugins disable PLUGIN_ID
sai plugins remove PLUGIN_ID
```

替换安装保留已保存设置和授权，新增能力需要再次明确授权。撤权选项依各包能力而定，普通 `enable` 不恢复撤销项。已有会话通过 `/plugins reload` 更新快照；已开始的调用使用原快照直到完成或取消。

卸载删除已安装源码，默认保留业务数据。各包说明数据位置和清理方法；保留旧源码包可以回退代码，数据回退需遵守对应格式。归档只收录清单和 Lua 源码，README、设置样本与图库等可选资源应单独分发，详见[分发说明](../../design/lua-plugins/distribution.md)。
