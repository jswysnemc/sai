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

`sai plugins run` 启动独立命令实例。命令可显式调用已授权的工具和模型；仅在调用模型时初始化客户端。写入命令沿用 CLI 权限模式和审计流程；需要时使用现有的 `--plan`、`--audited` 或 `--yolo` 顶层选项。

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

`--allow-http` 替换显式 HTTP 来源集合，可以重复传入。需要只读 POST 的包另用 `--allow-http-read-only-post` 授予清单中声明的精确端点，并同时提供所属来源。`--grant-declared` 明确授权当前清单中的全部能力，不能与分项参数组合；`--no-http` 仅撤销来源和只读 POST 授权，保留模型与工具授权。只设置 `--allow-http` 时不会保留旧的只读 POST 授权。实际可访问范围始终是声明与授权的交集。URL 路径、凭据、通配符和尾部 `/` 不属于合法来源声明。

调查、诊断等插件可声明 `"model": true` 和精确工具名称列表 `"tools": ["read_file", "web_search"]`，然后单独授权：

```sh
sai plugins enable investigation --allow-model --allow-tool read_file --allow-tool web_search
sai plugins enable investigation --no-model
sai plugins enable investigation --no-tools
```

`--allow-model` 与 `--no-model` 只修改模型授权。重复的 `--allow-tool` 构成新的工具集合，`--no-tools` 清空它；未指定的其他能力保持原值。外部插件启用本身不授予模型或工具权限。模型客户端继承当前 Agent 或子任务实际选择，地址和 API Key 不进入 Lua 配置。工具组合仍受 Agent 白名单及实时权限模式约束，接口与调用限制见 [Lua API](api.md)。

`sai plugins info <id> --json` 显示清单、安装位置和有效授权，不输出插件设置中的秘密值。

## 更新、禁用与移除

```sh
sai plugins install ./hello --replace
sai plugins disable hello
sai plugins remove hello
```

更新保留已有开关、设置和授权。新增来源、模型或工具声明不会自动扩大显式授权。管理命令使用文件锁、同目录暂存和原子配置替换；失败时恢复旧安装目录。

禁用、更新和移除在新的工具表或 `/plugins reload` 后生效。已经开始的调用和后台子任务使用原快照，直到完成或取消。移除会清除安装目录，并将配置置为禁用、撤销全部能力授权。内置插件只能禁用。

## 现有功能的兼容迁移

| 插件 | 工具名称 | 默认启用规则 |
| --- | --- | --- |
| `online-man` | `online_man_search`、`online_man_get_page` | 沿用主配置 `plugins.man.enabled` |
| `deepseek-status` | `query_deepseek_status` | 默认启用 |
| `archlinux` | `aur_search_packages`、`aur_get_package_info`、`archlinux_official_package_query`、`aur_check_status`、`archwiki_query` | 沿用主配置 `plugins.archlinux.enabled` |
| `fcitx-wiki` | `fcitx5_input_method_wiki_qurey` | 默认启用，保留旧名称中的 `qurey` 拼写 |
| `protondb` | `protondb_query` | 默认启用 |
| `web-search` | `web_search` | 沿用主配置 `plugins.web.enabled` |
| `linux-game-signals` | `gather_linux_game_compatibility_signals` | 沿用主配置 `plugins.linux_game_compatibility.enabled` |
| `linux-game-investigation` | `linux_game_compatibility` | 沿用主配置 `plugins.linux_game_compatibility.enabled` |
| `input-method-investigation` | `linux_input_method_diagnose` | 沿用主配置 `plugins.deep_diagnose.enabled` |
| `diagnostic-evidence` | `check_issue`、`diagnostic_app_probe` | 沿用主配置 `plugins.diagnostics.enabled` |
| `weather` | `get_weather` | 沿用主配置 `plugins.weather.enabled` |
| `exchange-rate` | `get_exchange_rate` | 沿用主配置 `plugins.exchange_rate.enabled` |
| `moegirl` | `query_moegirl` | 沿用主配置 `plugins.moegirl.enabled` |
| `package-advisor` | `review_aur_package`、`install_aur_package` | 沿用主配置 `plugins.package_advisor.enabled` |
| `reply-notification` | 无模型工具；提供 `preview` 命令和 `reply_end` 回调 | 默认启用，沿用主配置的通知和声音设置 |
| `image-generation` | `generate_image` | 沿用主配置 `plugins.image_generation.enabled` |
| `image-display` | `print_image` | 沿用主配置 `plugins.print_image.enabled` |
| `web-images` | `search_web_images` | 沿用主配置 `plugins.web_images.enabled` |

`plugins.jsonc` 的显式设置优先于上述默认值。内置包保留原工具名称；外部包不能使用内置插件 ID，也不能覆盖现有工具。已禁用的内置工具仍可在 Agent 设置中预先选择，但无法实际执行。

图片生成设置和显示设置分别归属两个包。生成包保存图片后，可按 `auto_print` 调用显示包；禁用显示包会跳过自动预览，预览失败不影响已保存的图片。二进制响应、匿名公开下载、输出目录和终端展示使用独立授权，详见[二进制接口](binary-api.md)。

网页搜图保留只读查询和普通下载两种模式。`web-images` 负责 DuckDuckGo/Bing 回退、排序去重、图片元数据、下载与视觉筛选；`auto_preview` 控制通过显示包预览，显式 false 覆盖旧设置。视觉使用主配置中的独立视觉供应商与模型，不随当前文本模型切换。`sai plugins enable web-images --no-vision` 可以单独撤销视觉能力，`--allow-vision` 恢复；筛选失败仍返回已经保存的图片及失败原因。

缓存目录默认为应用图片目录的 `web-images` 子目录，可通过 `cache_dir` 覆盖。搜索地址可通过 `duckduckgo_base_url` 和 `bing_base_url` 指向独立实例。更改目录或来源后，原显式授权保持固定；可先核对 `sai plugins info web-images --json`，再授权所需目录与来源。

查询工具和 `check_issue` 保持只读；`diagnostic_app_probe` 是明确执行应用的写入工具，不出现在只读工具目录中。ProtonDB 的 Algolia 搜索使用 GET，数字 App ID 的名称查询失败时保留原 ID，评论读取失败时仍返回评级。Fcitx 的 `include_page_excerpt=false` 完全使用包内规则；启用摘录时最多读取 512 KiB 页面，保留 Markdown 并截取 12,000 个 Unicode 字符。ArchWiki 页面同样保留 Markdown 链接。

### 答复通知

TUI 与 Web 的完成、中断和失败通知由 `reply-notification` 计算，系统通知与提示音分别控制。`notification.enabled`、`notification.sound` 继续提供默认值；显式插件设置优先。例如，将 `{"sound":false}` 保存为 JSON，再执行 `sai plugins configure reply-notification ./settings.json`，即可沿用旧通知开关并关闭提示音。

```sh
sai plugins run reply-notification preview '{"surface":"tui","status":"completed","locale":"zh-CN"}'
sai plugins enable reply-notification --no-notifications
sai plugins enable reply-notification --allow-notifications
sai plugins disable reply-notification
```

`preview` 只返回数据，不显示通知或播放声音；它可以在没有模型配置时运行。正式通知需要插件启用且取得 `notifications` 授权。撤销授权后，普通 `enable` 不会恢复它；通知开关与声音开关均为 false 时没有任何投递。

纯策略每次计算读取新设置，Web 设置页中的旧通知开关也会在下一次计算生效。外部策略包通过 `--allow-notifications` 单独授权，其回调只返回展示数据；接口与预算见 [Lua API](api.md#通知纯回调)。客户端不会因回放已经结束的历史轮次再次播放提示音。

### 天气、汇率与萌娘百科

三个包可以独立启停，普通与计划模式共用同一 Lua 实现。`weather` 通过 wttr.in 查询；`location` 留空时自动定位，城市名和机场代码保持原有用法。返回文本保留 `current weather(condition,temperature,wind,location): ` 前缀，单次正文最多 64 KiB。

`exchange-rate` 保留中文币种别名及原有汇率文本格式。旧 `plugins.exchange_rate.api_key` 和 `free_fallback_enabled` 继续提供默认值，插件 `settings` 按字段覆盖。例如，将以下对象保存到文件并通过 `sai plugins configure exchange-rate ./settings.json` 应用：

```json
{"free_fallback_enabled": false}
```

该配置沿用旧密钥并关闭免费回退；也可显式设置 `api_key`，空字符串会覆盖旧密钥。密钥沿用旧字段的字面字符串语义，查询时去除首尾空白。启停不会把旧密钥或默认值复制到 `plugins.jsonc`，后续旧设置修改在新实例或重载后生效。错误类型、null 或字符串形式的布尔值会在配置保存前失败。

有密钥时先访问配置接口；只有成功解析 JSON 但没有取得成功数值时，才在允许的情况下使用免费接口。HTTP、传输和 JSON 解析错误直接结束调用。来源限定为清单中的两个汇率服务，密钥和币种分别作为路径段编码，错误不回显带密钥的 URL 或远端正文。

`moegirl` 保留 `auto`、`search`、`page` 模式及 `zh`/`cn`、`uk`、`ja` 站点。自动模式优先显式标题，否则使用首个字符串搜索标题；无命中时读取原查询。搜索结果保留同一 JSON 数据，输出采用紧凑排版。REST 页面只有传输或正文大小失败时才回退解析 API，HTTP 状态错误直接失败；Markdown 正文按 20,000 个 Unicode 字符截断。

日文旧域名的 301 目标 `https://ja.moegirl.org.cn` 包含在缺省 HTTP 授权中。已有显式授权不会自动扩大，可核对 `sai plugins info moegirl --json` 后使用 `--grant-declared` 更新。

三者均受独立网络授权约束。例如 `sai plugins enable weather --no-http` 保留启用状态并撤销网络访问；再次普通启用不会恢复已撤销的授权。

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

### 游戏兼容性证据

`linux-game-signals` 收集 Steam、ProtonDB、Can I Play on Linux 和 AreWeAntiCheatYet 的游戏资料。可直接调用只读工具：

```sh
sai --plan __tool gather_linux_game_compatibility_signals '{"game":"Portal 2","issue":"multiplayer"}'
```

包内处理游戏别名、候选路径、页面摘要、可玩性判定及证据置信度；来源请求各限 20 秒，整个回调限 150 秒。失败来源保留尝试记录，成功取得的空页面或 JSON null 与请求失败分别记录。数据不足时 `needs_followup` 为 true。

结果中的 `verdict.traffic_light` 使用 `red`、`yellow`、`green` 文本值；中文 `label` 和原有判定条件不变。其他结果字段保持兼容，但 HTTP 错误措辞由插件宿主管理。界面可按文本状态使用自身图标组件。

`linux-game-investigation` 负责整个调查循环：组装提示、请求模型、逐项调用证据工具、控制预算、记录进度与统计，并整理最终报告。公开名称仍为 `linux_game_compatibility`，调用同一采集插件：

```sh
sai --plan __tool linux_game_compatibility '{"game":"Portal 2","issue":"multiplayer"}'
```

两个包各自保留独立开关。禁用 `linux-game-signals` 后，采集工具从可执行目录移除，调查入口在请求模型之前报告插件不可用。旧游戏开关提供两个包的默认值；显式插件启用设置分别优先，可单独保留证据采集。

调查包支持 `max_tool_steps`、`progress_mode`（`hidden`、`summary`、`full`）与 `language` 设置，省略时分别沿用旧游戏预算、工具显示模式和当前语言。`max_tool_steps=0` 表示不增加业务步数限制，宿主资源上限仍有效。工具预算耗尽、模型请求额度接近上限或消息数接近 256 条时，循环请求一次无工具最终报告。

结果保留 `final_report`、`stats` 和 `output_instruction`。每个模型响应只计入一次用量，修正旧版末次响应重复累加；缺失用量时估算本次消息与响应文本，并保留原统计字段及估算方法标签。概要进度区分工具成功和错误；详细进度最多 128 条，缩短过长预览并保留有效 JSON，展示额度耗尽不会停止调查。

### Linux 输入法调查

`input-method-investigation` 通过当前 Agent 或子任务的模型调查问题，使用同一工具目录中的 `check_issue`、Fcitx Wiki、知识库、文件读取和网页查询：

```sh
sai --plan __tool linux_input_method_diagnose '{"issue":"Steam 无法输入中文","target":"steam"}'
```

`issue` 必填，`target` 可省略；结果保留 `kind`、`issue`、`target`、`final_answer`、`stats` 和 `output_instruction`，没有目标时 `target` 为 JSON null。统计每个模型响应一次，修正原版重复累计末次用量的问题。单项探测失败或超时后，模型仍能根据已有证据继续调查并整理报告；摘要进度准确区分成功和错误。

包设置支持 `max_tool_steps`、`tool_timeout_ms`、`progress_mode` 与 `language`。缺省步数沿用主配置 `plugins.deep_diagnose.max_tool_steps`，`0` 表示不增加业务步数限制。缺省超时从旧 `tool_call_timeout_seconds` 转换，保留五秒下限并受 900 秒回调上限约束；显式 `tool_timeout_ms` 接受正整数毫秒。进度模式沿用当前显示配置，支持 `hidden`、`summary` 和 `full`。预算、超时和进度模式中的显式 `false` 属于类型错误，不会静默采用默认值。

例如将以下设置保存为 `input-method-settings.json`：

```json
{"max_tool_steps":12,"tool_timeout_ms":15000,"progress_mode":"summary"}
```

```sh
sai plugins configure input-method-investigation ./input-method-settings.json
sai plugins disable input-method-investigation
```

调查包可以独立启停，禁用后 `check_issue` 和 Fcitx Wiki 仍可单独使用。显式插件开关优先于旧输入法开关。调查只使用授权范围内的只读工具；模型消息不包含主会话历史或供应商凭据。模型、工具和消息预算的收尾规则与游戏调查一致，接口限制见 [Lua API](api.md)。

### 本地诊断证据

`diagnostic-evidence` 把 `check_issue` 的参数推断、Linux 九类采集、macOS 基础采集、输入法路径规则和报告格式迁入 Lua。Windows 自动模式仍返回 `unsupported`，不会执行 Linux 命令。

```sh
sai --plan __tool check_issue '{"area":"input_method","target":"your-app","depth":"quick"}'
sai plugins info diagnostic-evidence --json
sai plugins enable diagnostic-evidence --no-processes
```

普通取证不再自动执行目标的 `--version`。需要运行目标时，调用写入工具 `diagnostic_app_probe`，传入 `probe="version"` 或 `probe="launch"`、`target`，以及可选的 `area="app"` 或 `area="input_method"`。旧 `check_issue` 参数 `allow_launch_probe=true` 会在取证前返回明确错误，指向 `diagnostic_app_probe`，不会静默忽略启动意图。

启动探测沿用 `launch_timeout_seconds` 的 1–15 秒采样范围；目标已运行时跳过重复启动。长时间运行的目标使用既有 `run_command` 管理，报告返回 `facts.launch_probe.managed_task_id`，可以用 `background_command action=stop` 停止。禁用后台命令时，启动执行沿用宿主同步命令的超时策略，不承诺继续保留应用。

包设置为 `command_timeout_ms`、`max_stdout_chars` 和 `max_stderr_chars`。旧 `plugins.diagnostics` 设置仅提供缺省值：旧命令时长收窄到 1–120 秒，输出最多各 200,000 字符；显式毫秒设置支持 1–120,000，显式输出限制支持 0–200,000。非法类型或超限值在保存前失败。

文件、环境、进程分别授权。内置清单明确列出 `/proc`、系统模块目录、桌面入口目录、系统信息文件、输入法环境变量和固定只读模板；显式版本模板声明写入，启动依赖单独的 `run_command` 工具授权。撤销一类能力后，报告保留其他证据并记录缺失来源。完整接口见[系统能力说明](system-api.md)。

## 验证

```sh
cargo test -p sai-plugin-runtime --locked
cargo test -p sai --locked plugins::tests
cargo test --locked
```

运行时测试不依赖 Sai 配置；业务测试通过固定 HTTP 样本运行实际 Lua 源码。模型与工具集成测试用本地 SSE 服务驱动真实客户端和注册表，覆盖模型切换、权限、插件组合、递归、取消、单工具超时和完整调查。游戏报告有 45 组旧版 Rust 对照，输入法报告、Unicode 摘录和问题提示有 75 组对照。完整回归包含会话与后台交付流程，能发现新增异步包装对已有运行链路的影响。

完整接口见 [Lua API](api.md)，架构依据见 [架构说明](architecture.md)，迁移证据见 [迁移清单](migration.md)。
