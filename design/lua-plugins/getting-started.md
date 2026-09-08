# 开发与使用 Lua 插件

Sai 的插件包包含 `sai-plugin.json` 和 Lua 源码。工具、用户命令与生命周期监听器由同一个插件实例持有。无需修改 Rust 主配置类型，也无需为外部插件增加硬编码注册分支。

## 创建和安装

```sh
sai plugins init hello ./hello
sai plugins check ./hello
sai plugins install ./hello
sai plugins enable hello
sai plugins list
sai plugins commands
sai plugins run hello stats
```

生成的包包含 `greet` 工具、`stats` 命令以及 `turn_start` 监听器。安装只复制经过验证的清单和 Lua 文件；工作区里的插件文件不会自行执行，新安装的包默认禁用。

外部工具名称为 `lua__hello__greet`。启用后，工具会出现在 Agent 设置的 Lua 插件分组中。使用工具白名单的 Agent 还需要选中该工具；使用渐进加载的 Agent 仍须先调用 `load`。

`sai plugins run` 启动独立命令实例，不调用语言模型。写入命令沿用 CLI 权限模式和审计流程；需要时使用现有的 `--plan`、`--audited` 或 `--yolo` 顶层选项。

## 在 TUI 会话中使用

```text
/plugins
/plugins reload
/plugin hello/stats
```

`/plugins` 显示本会话实际加载的插件版本和命令。`/plugin` 与本会话的工具共享 Lua 状态，执行期间支持进度、窗口刷新以及 Esc/Ctrl+C 取消。命令结果供用户阅读，不会隐式追加到模型上下文。

`/plugins reload` 重新发现配置和源码，重建本地工具及子任务注册，保留当前对话、模型和已经发现的 MCP 工具。没有变化的插件继续使用原状态；源码、设置或授权改变后创建新实例。旧的后台预热结果不会覆盖新快照。

Web、CLI 对话和子任务使用同一工具注册适配。Web 的下一次工具表构造会读取已保存的插件配置。版本 1 的直接插件命令提供 CLI 和 TUI 入口。

## 配置与授权

插件配置位于 `sai paths` 所示配置目录的 `plugins.jsonc`，与主配置分开。例如，为模板设置问候语：

```json
{
  "api_version": 1,
  "plugins": {
    "hello": {
      "enabled": true,
      "settings": { "greeting": "你好" },
      "grants": { "http": [] }
    }
  }
}
```

也可把 `{"greeting":"你好"}` 保存为 JSON 文件，再执行：

```sh
sai plugins configure hello ./settings.json
```

Lua 通过 `sai.config` 读取自己的设置，无法读取整份 Sai 配置。初始化应只加载包内模块、注册接口；依赖网络或必需业务参数的操作放在执行函数中。

HTTP 插件必须在清单中声明精确来源，例如 `https://api.example.com`。外部插件启用时不会自动获得网络权限：

```sh
sai plugins enable network-plugin --allow-http https://api.example.com
sai plugins enable network-plugin --allow-http https://api.example.com --allow-http-read-only-post https://api.example.com/search
sai plugins enable network-plugin --grant-declared
sai plugins enable network-plugin --no-http
```

`--allow-http` 替换显式 HTTP 来源集合，可以重复传入。需要只读 POST 的包另用 `--allow-http-read-only-post` 授予清单中声明的精确端点，并同时提供所属来源。`--grant-declared` 明确授权当前清单中的全部网络能力；`--no-http` 撤销来源和只读 POST 授权。只设置 `--allow-http` 时不会保留旧的只读 POST 授权。实际可访问范围始终是声明与授权的交集。URL 路径、凭据、通配符和尾部 `/` 不属于合法来源声明。

`sai plugins info <id> --json` 显示清单、安装位置和有效授权，不输出插件设置中的秘密值。

## 更新、禁用与移除

```sh
sai plugins install ./hello --replace
sai plugins disable hello
sai plugins remove hello
```

更新保留已有开关、设置和授权。新的来源声明不会自动扩大授权。管理命令使用文件锁、同目录暂存和原子配置替换；失败时恢复旧安装目录。

禁用、更新和移除在新的工具表或 `/plugins reload` 后生效。已经开始的调用和后台子任务使用原快照，直到完成或取消。移除会清除安装目录，并将配置置为禁用、撤销网络授权。内置插件只能禁用。

## 现有功能的兼容迁移

| 插件 | 工具名称 | 默认启用规则 |
| --- | --- | --- |
| `online-man` | `online_man_search`、`online_man_get_page` | 沿用主配置 `plugins.man.enabled` |
| `deepseek-status` | `query_deepseek_status` | 默认启用 |
| `archlinux` | `aur_search_packages`、`aur_get_package_info`、`archlinux_official_package_query`、`aur_check_status`、`archwiki_query` | 沿用主配置 `plugins.archlinux.enabled` |
| `fcitx-wiki` | `fcitx5_input_method_wiki_qurey` | 默认启用，保留旧名称中的 `qurey` 拼写 |
| `protondb` | `protondb_query` | 默认启用 |
| `web-search` | `web_search` | 沿用主配置 `plugins.web.enabled` |

`plugins.jsonc` 的显式设置优先于上述默认值。内置包保留原工具名称；外部包不能使用内置插件 ID，也不能覆盖现有工具。已禁用的内置工具仍可在 Agent 设置中预先选择，但无法实际执行。

这些查询工具均为只读。ProtonDB 的 Algolia 搜索使用 GET，数字 App ID 的名称查询失败时保留原 ID，评论读取失败时仍返回评级。Fcitx 的 `include_page_excerpt=false` 完全使用包内规则；启用摘录时最多读取 512 KiB 页面，保留 Markdown 并截取 12,000 个 Unicode 字符。ArchWiki 页面同样保留 Markdown 链接。

### 网页搜索设置

`web-search` 包含 TinyFish、Tavily、Firecrawl、AnySearch、SearXNG 和 DuckDuckGo。自动模式沿用此顺序；显式指定供应商时仅尝试该供应商，`script` 保留为 DuckDuckGo 的旧别名。通用网页读取仍使用 `web_fetch`。

旧版 `plugins.web` 字段继续提供默认设置，`plugins.jsonc` 的 `web-search.settings` 按字段覆盖。例如：

```json
{
  "api_version": 1,
  "plugins": {
    "web-search": {
      "enabled": true,
      "settings": {
        "default_provider": "tavily",
        "tavily_api_keys": ["$env:TAVILY_API_KEY"],
        "tavily_search_depth": "advanced",
        "timeout_seconds": 60
      }
    }
  }
}
```

设置使用原有字段名称。宿主只向该内置包传递搜索配置；API Key 按首个非空配置值、显式 `$env:` 引用和原供应商环境变量的规则解析。解析结果保存在运行时快照中，启停和配置操作不会将其写回磁盘。环境变量变化在新实例或显式重载后生效，外部插件不能借此读取 Sai 配置或任意环境变量。

五个可配置服务地址继续支持自定义 HTTP(S) 端点，地址同时受插件 URL 规则约束。兼容层将地址转换为来源和必要的 POST 查询端点，不在清单或授权展示中保留查询参数。内置包没有显式 `grants` 时沿用内置授权；已有显式授权不会因地址修改自动扩大。更换来源后，可核对 `sai plugins info web-search --json` 并使用 `--grant-declared` 更新授权。

新增搜索供应商的实现、选择规则和参数放在 Lua 包内，来源与查询端点放在清单中；新增设置放入插件自己的 `settings`，无需增加 AppConfig 字段。

## 验证

```sh
cargo test -p sai-plugin-runtime --locked
cargo test -p sai --locked plugins::tests
cargo test --locked
```

运行时测试不依赖 Sai 配置；业务测试通过固定 HTTP 样本运行实际 Lua 源码。完整回归包含会话与后台交付流程，能发现新增异步包装对已有运行链路的影响。

完整接口见 [Lua API](api.md)，架构依据见 [架构说明](architecture.md)，迁移证据见 [迁移清单](migration.md)。
