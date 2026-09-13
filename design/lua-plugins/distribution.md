# 打包、发布与维护 Lua 插件

`sai plugins pack` 将通过加载和注册检查的源码快照写入标准 `tar.gz`。使用者先解压，再执行 `check` 与 `install`；打包和安装都不需要模型配置。

## 创建分发包

```sh
sai plugins pack ./project-notes
sai plugins pack ./project-notes --output ./releases/project-notes-0.1.0.tgz
sai plugins pack ./project-notes -o ./releases/project-notes.tar.gz --json
```

默认输出在当前工作目录，名称为 `<id>-<version>.tar.gz`，版本和 ID 来自清单。`--output`（简写 `-o`）接受以 `.tar.gz` 或 `.tgz` 结尾的 UTF-8 路径，并创建缺失的父目录。已有文件、目录和符号链接都会导致失败；打包没有 `--replace`，再次发布应选择新版本或新路径。

普通输出显示归档绝对路径与 SHA-256。`--json` 返回以下字段：

| 字段 | 含义 |
| --- | --- |
| `id`、`version`、`api_version` | 已验证清单中的插件 ID、SemVer 和宿主 API 版本 |
| `path` | 最终归档的绝对路径，父目录已规范化 |
| `bytes` | 完整压缩文件的字节数 |
| `sha256` | 完整压缩文件的 SHA-256，小写十六进制 |
| `files` | 归档中普通文件的有序路径，包含插件 ID 根目录 |

包的布局如下；项目笔记示例实际包含三个 Lua 模块：

```text
project-notes/
├── sai-plugin.json
├── history.lua
├── init.lua
└── inspect.lua
```

归档先保存规范清单，再按路径顺序保存全部包内 Lua 源码。它与安装器采用相同文件选择规则：忽略 `.git` 目录，排除 README、`.env`、普通设置 JSON、图片和原生库等文件。包根目录必须是真实目录；扫描范围内的符号链接会导致失败。源码内写死的密钥仍属于源码内容，发布前应自行移除。

清单最多 64 KiB；Lua 源码最多 128 个文件、合计 4 MiB，目录嵌套最多 12 层。清单保存时通常使用带末尾换行的格式化 JSON；排版导致超限时改用紧凑 JSON，补全默认字段后仍超限则拒绝打包或安装。静态数据可写成 Lua 模块，外部数据通过已授权的公共接口获取。

包内源码路径按真实目录层级记录，统一使用 `/` 分隔。文件和目录的名称不能包含字面反斜杠，也须满足清单的跨平台路径规则。例如 Linux 中的 `modules\check.lua` 不是 `modules/check.lua` 的替代写法；检查、打包和安装会拒绝前者，并报告非法路径。将模块放入真实的 `modules/` 目录，不通过替换名称字符合并文件。包外父目录的名称不受此规则限制。

文件权限固定为 `0644`，用户、组及修改时间为零；gzip 不记录源文件名和当前时间。同一程序对同一清单和 Lua 快照打包时，源码目录位置、文件时间和输出文件名不影响归档字节。不同压缩库或宿主版本之间不承诺逐字节一致。

`pack` 与 `check` 执行同样的受限初始化：读取默认空设置、加载模块并验证注册；不调用业务命令、不授予权限，也不证明外部服务可用。归档不包含用户的 `plugins.jsonc`、设置或授权。先在同目录暂存，完整压缩、同步并计算摘要后发布，失败会清理暂存文件。

## 提供发布说明

README、发行说明与配置样例需要单独提供；`pack` 不将这些文件写入归档。发布说明至少包含：

- 插件版本、`api_version`、所需接口，以及实际验证的 sai 版本和构建标识。同一开发版本号的构建可用程序 SHA-256 区分。
- 支持的平台、外部程序或服务、安装和执行示例，以及每项能力的用途与最小授权命令。
- 设置字段、错误处理、持久数据格式、升级与回退边界，以及卸载前的显式清理方法。
- 归档文件名和 SHA-256，供接收者核对传输内容。摘要不提供发布者身份认证。

当前没有宿主版本范围、依赖求解或签名字段。`api_version = 1` 不代表所有历史 v1 程序都有新增接口；清单不能加入未定义字段。版本与弃用规则见[兼容说明](compatibility.md)。

## 解压、检查与安装

以项目笔记 `0.1.0` 版本为例，使用系统的 tar 工具解压到一个新目录：

```sh
mkdir ./unpacked
tar -xzf ./project-notes-0.1.0.tar.gz -C ./unpacked
sai plugins check ./unpacked/project-notes
sai plugins install ./unpacked/project-notes
```

`install` 接受解压后的目录，不直接读取压缩文件或远程 URL。接收方的宿主检查清单、API 版本、源码和注册；同 ID 已安装时需要显式 `--replace`，没有保留的内置插件 ID。归档根目录下的清单决定插件身份，修改压缩文件名不会改变它。

新安装的包默认禁用。依照插件说明配置并明确授权后执行；以下命令从含 `notes/README.md` 的项目目录运行：

```sh
sai plugins enable project-notes --allow-read-path notes
sai --plan plugins run project-notes inspect
sai plugins info project-notes --json
```

项目笔记的可选设置、持久记录与独立存储授权见[示例说明](../../examples/lua-plugins/project-notes/README.md)。实际权限始终取清单声明与用户授权的交集。

## 替换、撤权与卸载

将新版本解压到另一目录，检查后替换安装：

```sh
mkdir ./updated
tar -xzf ./project-notes-0.2.0.tar.gz -C ./updated
sai plugins check ./updated/project-notes
sai plugins install ./updated/project-notes --replace
sai plugins info project-notes --json
```

替换更新保留开关、设置和显式授权，不自动授予新声明的能力。CLI 下次调用加载新版本；已有 TUI 会话使用 `/plugins reload`。已开始的调用继续使用原快照直至完成或取消。

保留旧源码归档和设置备份后，可通过同样的 `install --replace` 恢复旧版源码。插件自行写入的数据不会随源码回退；数据兼容由插件的格式版本和升级策略决定。

```sh
sai plugins enable project-notes --no-file-read
sai plugins enable project-notes --no-plugin-storage
sai plugins disable project-notes
sai plugins remove project-notes
```

普通启用不会恢复撤销的授权。禁用和卸载保留持久记录；需要删除时，先按插件说明执行显式清理命令，再撤权和移除。`remove` 清除安装目录、禁用该包并撤销其全部授权。

## 定位打包失败

| 现象 | 处理方式 |
| --- | --- |
| 输出路径已存在 | 使用新版本或新的输出文件名，已有内容不会被覆盖 |
| 输出扩展名错误 | 使用 `.tar.gz` 或 `.tgz`，完整路径必须是 UTF-8 |
| 清单或源码超限 | 缩减声明或拆分业务；仅删除空白不一定解决默认字段膨胀 |
| `invalid plugin source path` | 按错误中的包内路径修正非法名称；反斜杠不能代替真实子目录，先确认两份同名模块各自的内容 |
| 初始化、模块或注册失败 | 按 `check` 相同诊断修复源码；初始化不能依赖网络和用户必填设置 |
| 解压后安装拒绝 | 使用满足所需 API 的宿主，检查包身份及安装路径；替换已有同 ID 包须显式使用 `--replace` |
| 更新后行为未变化 | 确认 `install --replace` 的目录与 `info` 版本；已有 TUI 会话重新加载 |

## 复现命令行验收

仓库内可对已经构建的 sai 执行：

```sh
uv run --no-project python scripts/plugin-smoke/verify_distribution.py \
  --binary /absolute/path/to/sai \
  --report target/plugin-distribution-validation/cli-report.json
```

脚本使用 Linux XDG 隔离和仓库外源码，核对归档内容、摘要、固定元数据、解压安装、执行、更新不扩权、撤权卸载及 64 KiB 清单边界。结束后清理临时目录；报告保留程序摘要、命令结果和耗时。此验收不访问模型或外网，也不代表其他平台已经验证。

源码路径冲突及失败更新的数据保留可单独复验：

```sh
uv run --no-project python scripts/plugin-smoke/verify_source_paths.py \
  --binary /absolute/path/to/sai \
  --report target/plugin-source-path-validation/cli-report.json
```
