# 项目笔记插件

这个独立 Lua 包读取项目中的 UTF-8 笔记，返回 SHA-256 和短预览，并可将检查记录保存到本插件的持久存储。只需要支持下述 v1 接口的 sai 程序，不需要模型、网络或修改宿主源码。

## 安装与最小授权

把本目录复制或解压到任意目录，例如 `~/extensions/project-notes`。从你的项目目录执行；该目录应有 `notes/README.md`，正文不超过 65536 字节：

```sh
sai plugins check ~/extensions/project-notes
sai plugins install ~/extensions/project-notes
sai plugins enable project-notes --allow-read-path notes
sai --plan plugins run project-notes inspect
```

`check` 应报告一个工具、五个命令、一个事件。`install` 只复制清单和三个 Lua 模块，新安装的包默认禁用。工具名称是 `lua__project-notes__inspect`，可以在 Agent 工具目录中选择；命令可以直接执行，不需要先发起模型对话。

返回示例包含 `path`、`bytes`、`sha256`、`preview`。路径是本次任务目录下的真实文件位置，SHA-256 对应完整读取的正文；超过读取上限时明确失败，不对截断内容冒充完整摘要。

清单只声明 `system.read_paths = ["notes"]` 和 `system.plugin_storage = true`。上面的授权仅开放前者。普通 `enable` 不自动授权；文件读取权限不会顺带开放私有存储、项目写入、网络或模型。

## 配置与保存记录

把以下设置写入 `project-notes-settings.json`：

```json
{"path":"notes/README.md","preview_chars":120}
```

```sh
sai plugins configure project-notes ./project-notes-settings.json
sai plugins info project-notes --json
sai --plan plugins run project-notes inspect
sai plugins enable project-notes --allow-plugin-storage
sai --yolo plugins run project-notes remember
sai --plan plugins run project-notes latest
```

`path` 默认 `notes/README.md`，`preview_chars` 默认 160，范围为 1–1000 的整数。未知设置、错误类型和非法范围会在 `configure` 保存前失败。设置不能扩大清单和用户授权：改为 `outside.txt` 后，检查会因越出 `notes` 目录而失败。

`remember` 是显式写入命令。上述 `--yolo` 允许这次命令保存私有记录；交互使用时也可省略它并沿用 Sai 的权限确认。`--plan` 会拒绝保存，即使已经授予存储能力。`latest` 可在另一个进程、会话或工作目录读取同一插件最近保存的记录，缺失时返回 `{"record":null}`。记录包含检查时间、路径、摘要、字节数和预览；它不会修改项目文件。

## 模块与生命周期

```text
project-notes/
├── sai-plugin.json  身份、资源上限和两项能力声明
├── init.lua         工具、命令和观察事件注册
├── inspect.lua      设置校验、文件读取、摘要与预览
└── history.lua      私有记录的保存、读取和删除
```

`inspect` 工具与同名命令复用一个实现。`remember/latest/forget` 展示持久存储的写入、读取和删除。`tool_result` 监听器只统计宿主确认成功的 `inspect` 工具调用；`stats` 命令读取当前实例的计数，事件不持久化记录或改变工具结果。

每次 `plugins run` 创建独立实例，因此单独执行 `stats` 得到零。TUI 中模型调用笔记工具后，`/plugin project-notes/stats` 可以观察同一实例的计数。`/plugin project-notes/inspect` 是用户命令，不计入模型工具计数。

初始化仅校验自身设置并注册接口。文件、存储和时间组合的业务在回调中执行；插件无需在安装时拥有项目目录或授权。

函数注释中的 `SaiContext` 等类型来自[LuaLS 声明](../../../design/lua-plugins/editor-support.md)。将声明目录配置为编辑器库即可使用，无需在插件中加载额外模块。

## 更新、重载与卸载

修改源码，按 SemVer 更新清单中的 `version`，再执行：

```sh
sai plugins check ~/extensions/project-notes
sai plugins install ~/extensions/project-notes --replace
sai --plan plugins run project-notes inspect
```

修改开发目录不会改变已经安装的快照。替换安装保留开关、设置和显式授权，新增能力需要另行授权。CLI 下次调用加载新版本；已有 TUI 会话执行 `/plugins reload`，未变化的实例保留计数，源码、设置或授权改变的实例重新初始化。已经开始的调用使用原快照直至完成或取消。

```sh
sai plugins enable project-notes --no-file-read
sai plugins enable project-notes --no-plugin-storage
sai plugins disable project-notes
sai plugins remove project-notes
```

撤权后普通 `enable` 不会恢复权限。禁用和卸载不会删除持久数据；需要清除时，应在卸载前授予插件存储并执行 `sai --yolo plugins run project-notes forget`。再次读取 `latest` 应返回空记录。

使用 `sai plugins pack ~/extensions/project-notes` 在当前目录生成 `<id>-<version>.tar.gz`，也可用 `--output` 指定新路径。归档只包含清单和三个 Lua 模块；本说明与发行信息另行提供。接收者先解压再安装，完整命令与摘要字段见[分发说明](../../../design/lua-plugins/distribution.md)。保留旧源码包可以通过 `install --replace` 恢复旧版；插件数据需由插件自己的格式兼容逻辑处理。

## 定位失败

| 现象 | 原因或检查方法 |
| --- | --- |
| 找不到命令 | 检查是否启用、加载诊断以及 `sai plugins commands` |
| 文件读取拒绝 | `info --json` 中有效读取授权应包含字面值 `notes`；从含 `notes/` 的项目目录调用 |
| `latest` 或 `remember` 拒绝 | 另需 `--allow-plugin-storage`；`remember` 还需要允许写入的模式 |
| `file exceeds 65536 bytes` | 文件超出示例上限，当前调用没有保存记录 |
| UTF-8 或路径错误 | 使用有效 UTF-8 普通文件，路径保持在已授权目录中 |
| 配置仍是旧值 | 失败配置不会覆盖旧值；成功配置在新实例或重载后生效 |

## 复现验收

在 sai 源码仓库中可对已经构建的程序执行：

```sh
uv run --no-project python scripts/plugin-smoke/verify.py \
  --binary /absolute/path/to/sai \
  --report target/external-plugin-validation/cli-report.json
```

该脚本目前使用 Linux XDG 隔离，在仓库外创建包、项目与配置目录，不修改用户配置。它完成 `init → check → install → configure/enable → run → update → revoke → disable/remove`，验证真实命令结果并记录程序摘要、退出状态和耗时。运行结束清理临时目录。TUI 的实例继续和重载由 `external_workflow` 集成契约验证；本脚本不启动真实 TUI、Web 或模型。
