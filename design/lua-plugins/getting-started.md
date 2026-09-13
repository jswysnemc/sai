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

访问已知 HTTP 服务的插件应在清单中声明精确来源，例如 `https://api.example.com`。外部插件启用时不会自动获得网络权限：

```sh
sai plugins enable network-plugin --allow-http https://api.example.com
sai plugins enable network-plugin --allow-http https://api.example.com --allow-http-read-only-post https://api.example.com/search
sai plugins enable network-plugin --grant-declared
sai plugins enable network-plugin --no-http
```

`--allow-http` 替换显式 HTTP 来源集合，可以重复传入。需要只读 POST 的包另用 `--allow-http-read-only-post` 授予清单中声明的精确端点，并同时提供所属来源。`--grant-declared` 明确授权当前清单中的全部能力，不能与分项参数组合；`--no-http` 撤销来源、只读 POST 和任意来源读取授权，保留模型与工具授权。只设置 `--allow-http` 时不会保留旧的只读 POST 授权。实际可访问范围始终是声明与授权的交集。URL 路径、凭据、通配符和尾部 `/` 不属于合法来源声明。

读取用户任意指定 URL 的插件可独立声明 `http_read_any`，通过 `--allow-http-read-any` 授权、`--no-http-read-any` 撤销。它包含本地服务，只开放 GET/HEAD；适用场景、重定向和错误正文选项见 [HTTP 公共契约](http-api.md)。

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

禁用、更新和移除在新的工具表或 `/plugins reload` 后生效。已经开始的调用和后台子任务使用原快照，直到完成或取消。移除会清除安装目录，并将配置置为禁用、撤销全部能力授权。所有业务示例均使用普通安装身份。

## 独立扩展示例

[项目笔记示例](../../examples/lua-plugins/project-notes/README.md)组合受授权文件读取、文本和摘要、插件持久存储以及工具结果事件。复制到任意仓库外目录后即可安装，不需要模型配置、网络、内置身份或修改 Rust。

只读取时，仅授予清单中的 `system.read_paths = ["notes"]`；需要跨会话保存记录时再授予 `system.plugin_storage`。文件路径相对于每次调用的工作目录，设置不会改变授权范围。完整命令、预期结果和失败定位均在示例说明中。

[URL 预览示例](../../examples/lua-plugins/url-preview/README.md)组合任意来源 GET、跳转与错误正文控制，以及有界 HTML 摘录。两个示例和入门模板均可使用[LuaLS 类型提示](editor-support.md)。

## 定位错误

| 现象 | 检查方式 |
| --- | --- |
| `check` 报清单、语法、模块或注册错误 | 按错误中的插件 ID、文件和 Lua 行号修改源码；初始化只能校验设置和注册，不能做 I/O |
| `configure` 失败 | 设置必须是 JSON 对象；插件在初始化中校验类型、范围和未知字段，失败保留原设置 |
| 包已安装但命令或工具不存在 | 查看 `plugins info <id> --json` 的启用状态及 `diagnostics`，再查看 `plugins commands` |
| 能力已声明但访问遭拒 | 核对 `effective_grants`；普通启用不授予权限，工具白名单和计划模式还会继续收窄权限 |
| 相对路径没有找到文件 | 核对启动命令所在目录及 `ctx.workdir`；路径相对于任务目录，而非源码包或安装目录 |
| 修改源码后仍是旧行为 | 执行 `install --replace`；TUI 再执行 `/plugins reload`，独立 CLI 命令在下次执行时重新加载 |
| 超时、内存、指令或输出超限 | 先缩小单次工作量，按[资源契约](api.md#资源范围)核对请求与清单限制 |
| `pcall` 得到宿主错误 | 错误文本用于诊断；当前未提供稳定错误码，不依赖完整错误文案控制业务 |

`check` 只校验加载与注册，不执行每条业务分支、不批准权限，也不证明外部服务可用。`info --json` 不输出设置值；处理业务错误时同样避免输出插件自己的密钥。

## 接口索引与分发

- [能力清单](capability-matrix.md)：按场景查找公共接口、调用阶段、权限和支持入口
- [Lua API v1](api.md)：清单、注册、上下文、模型、工具、HTTP 与资源契约
- [打包与分发](distribution.md)：创建 tar.gz、核对摘要、解压安装和维护
- [兼容与维护](compatibility.md)：API 版本、弃用、错误分类与升级规则
- [编辑器支持](editor-support.md)：入门接口的 LuaLS 声明与配置
- [独立示例](../../examples/lua-plugins/project-notes/README.md)：可直接修改的多模块包及本地发布方法
- [旧版迁移说明](bundled-plugins.md)：工具名称、独立设置、数据与入口迁移

包的必需文件只有 `sai-plugin.json` 与 Lua 源码。可以直接分发源码目录，也可生成标准压缩包：

```sh
sai plugins pack ./hello
sai plugins pack ./hello --output ./releases/hello-0.1.0.tar.gz --json
```

默认输出当前目录下的 `<id>-<version>.tar.gz`，已有输出不会被覆盖。打包器和安装器只保存清单与包内 Lua 文件，不包含 README、图片、二进制依赖、用户设置或授权；需要的静态数据应组织为 Lua 模块，外部数据使用已授权的公共接口获取。README 与发行说明另行提供。接收者先解压，再使用 `check` 和 `install`；完整格式、SHA-256 字段和安装命令见[分发文档](distribution.md)。

发布说明应列出插件版本、实际验证的 sai 版本或构建、`api_version`、所用能力、平台和外部依赖。`api_version = 1` 是清单协议要求；同为 v1 的早期构建不一定包含后来增加的接口。目前没有宿主版本范围字段、依赖自动安装或签名校验。不要把这些字段写入清单，未知字段会报错。

更新前保留旧源码包及自己的设置备份。更新使用 `install --replace`，新能力由用户另行授权；需要恢复旧版时重新安装旧包，不自动回退插件自行写入的数据。禁用、撤权和卸载保留插件持久记录；有清理需求的插件应提供显式清理命令，并在卸载前执行。
