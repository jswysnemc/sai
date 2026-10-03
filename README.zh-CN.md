# Sai

**高性能终端与桌面 AI 编程工作台**
多协议 LLM 接入 · 系统级沙盒与 Plan 模式 · 渐进式工具加载 · Web 工作台 · 长期记忆 · 跨平台

[English](README.md) | 简体中文

[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-blueviolet)](https://github.com/jswysnemc/sai)
[![CI Linux](https://img.shields.io/github/actions/workflow/status/jswysnemc/sai/linux.yml?branch=main&label=CI%20Linux)](https://github.com/jswysnemc/sai/actions/workflows/linux.yml)
[![CI Windows](https://img.shields.io/github/actions/workflow/status/jswysnemc/sai/windows.yml?branch=main&label=CI%20Windows)](https://github.com/jswysnemc/sai/actions/workflows/windows.yml)
[![CI macOS](https://img.shields.io/github/actions/workflow/status/jswysnemc/sai/macos.yml?branch=main&label=CI%20macOS)](https://github.com/jswysnemc/sai/actions/workflows/macos.yml)

[为什么是 Sai](#为什么是-sai) · [界面预览](#界面预览) · [核心能力](#核心能力) · [安装指南](#安装指南) · [快速上手](#快速上手) · [CLI 命令参考](#cli-命令参考) · [系统架构](#系统架构) · [存储布局](#存储与目录布局) · [常见问题](#常见问题) · [致谢与协议](#致谢与协议)

---

## 为什么是 Sai

Sai 是使用 Rust 编写的高性能 AI 编程助手与桌面工作台。它将大语言模型的推理能力与操作系统工具、OS 级隔离沙盒、长期记忆、现代 Web 界面及外部通讯平台深度整合，既能作为终端交互式 REPL 与单轮问答工具，也是功能完备的本地与远程代码开发工作台。

本项目源于 [Miyu](https://github.com/SHORiN-KiWATA/Miyu)，在保留上游优秀设计的基础上进行了深度架构重构与能力扩充，全面强化跨平台能力，并在权限沙盒、Plan 规划工作流、Web 工作台、渐进式上下文管理与工具路由上实现了持续演进。

- **深度系统集成与全能工具链**：读写文件、精细补丁编辑、命令生命周期管控、代码全局检索、内置只读网络能力与浏览器操作。
- **三协议自适应引擎**：全面适配 OpenAI Chat、OpenAI Responses 与 Anthropic Messages 协议，支持各大主流供应商与本地兼容网关。
- **严格的安全沙盒与审计**：提供操作系统级写入隔离（Linux bubblewrap / macOS Seatbelt）、敏感凭证拦截与独立 Plan 模式。
- **现代多端界面**：同时支持带全屏转录/折叠能力的终端 TUI，以及集成 Monaco 编辑器、xterm 终端、内建浏览器与 Git 审查面板的 Web 工作台。
- **高能效上下文与 Jev 路由**：通过按需渐进式加载（Progressive Tool Loading）与 Jev 动态能力分发，维持高 Prompt 缓存命中率与长会话稳定性。

---

## 界面预览

### Web 编程工作台

集成会话时间线、任务计划执行流、Monaco 代码编辑、内建浏览器视口、Git 变更审查与提示词模板。

![Web 编程工作台](pics/web.png)

![源代码管理与差异审查](pics/web1.png)

### 终端交互式 REPL

全屏转录视图、代码与思考折叠、行内公式与图表流式输出，支持 OSC 52 鼠标拖拽复制。

![Sai REPL 对话与流式渲染](pics/repl.png)

### 终端配置 TUI

运行 `sai config` 启动分层配置界面，涵盖供应商、模型参数、Agent 人格、工具开关与沙盒策略。

![配置 TUI 主菜单](pics/config.png)

![Agent 工具与 Skills 勾选清单](pics/skills.png)

---

## 核心能力

### 多协议 LLM 引擎与深度推理控制

- **三协议自适应接入**：自动检测或手动指定 OpenAI Chat、OpenAI Responses、Anthropic Messages 协议；提供 opencode Zen、OpenAI、Anthropic 内置配置，支持任意兼容端点与多 API Key 自动负载均衡。
- **细粒度思维链控制**：支持 7 档思考强度（`auto`、`none`、`low`、`medium`、`high`、`xhigh`、`max`），深度兼容 DeepSeek 思维链、OpenAI Reasoning Effort 及 Anthropic Thinking 协议。
- **流式富文本渲染**：终端与前端统一实现 Token 级流式输出，原生支持 KaTeX 数学公式、Mermaid 流程与时序图、Syntect 语法高亮及准确的 Token 计量。
- **上下文自动压缩**：对话超出预算时自动调用专用压缩模型进行信息归纳；被裁剪轮次进入 `evicted_context.db`，可由记忆工具联想检索。

### 权限控制、系统级沙盒与 Plan 模式

- **三层权限模式**：
  - `Yolo`：全自动执行所有被授权工具。
  - `Audited`：限制在工作区沙盒内运行，敏感路径拦截，逐次操作人工确认并追加 `permission-audit.jsonl` 审计日志。
  - `Auto-audit`：LLM / Jev 自动安全检测与人工审核双轨并行，兼顾效率与安全。
- **系统级工作区沙盒**：
  - Linux 环境通过 `bubblewrap` 强制写入限制在当前工作区；
  - macOS 环境调用原生 `Seatbelt` (`sandbox-exec`) 建立内核级防护；
  - Windows 环境启用严格路径白名单与敏感凭据防护拦截；
  - 支持 `/sandbox` 指令随时检查当前运行后端的沙盒策略与可写根目录。
- **独立的只读 Plan 模式**：
  - 键入 `/plan` 或模型调用 `enter_plan_mode` 进入，从权限模式中完全解耦；
  - 在只读沙盒中进行代码通读、现状分析与网络检索，调用 `ask_question` 与用户交互澄清需求；
  - 编写包含实施目标、文件范围、执行步骤与验证方式的完整计划，调用 `exit_plan_mode` 提交；
  - 终端与 Web 呈现 Markdown 审阅卡片，用户点击批准（`Approve and implement`）后自动恢复原权限模式并继续实施。

### 渐进式工具系统与 Jev 动态路由

- **渐进式加载（Progressive Tool Loading）**：初始仅暴露极简基础工具，模型根据任务自主调用 `load` 激活特定工具组或 Skill，持久化至 `loaded-tools.json`，显著降低上下文污染与幻觉几率。
- **Jev 动态能力路由器**：在用户提问阶段预选所需能力，按需向模型暴露工具契约与 Skill 指南，保证系统提示词前缀稳定，充分利用厂商的 Prompt 缓存机制。
- **原生基础工具链**：
  - 文件精准编辑：行级精确定位替换（`str_replace`）、只读探测（`read_file`）、全量写入（`write_file`）、安全移入回收站（`trash_path`）；
  - 命令与后台进程：支持前台运行与后台任务管理（`run_command` / `background_command`），长任务自动降级与状态通知；
  - 内置网络探测：提供原生只读 `web_search`（内置 TinyFish、Tavily、Firecrawl、AnySearch、SearXNG、DuckDuckGo 故障转移）与 `web_fetch`（网页内容智能抽取，无须 Python 依赖）；
  - 交互式问答（`ask_question`）：结构化单选、多选与自定义补充输入。
- **子智能体（Subagents）体系**：
  - 独立上下文循环与执行步数上限（`max_steps`）；
  - 写任务自动在 Git 仓库创建 `.sai-subagents` Worktree 隔离开发，完成验证后合并回当前工作区分支；
  - 支持 persistent 常驻模式，通过主 Agent 追加消息（`/msg`）协同作业。
- **开放插件与协议扩展**：
  - 原生 MCP 支持：支持 stdio 与 http 传输协议，自动以 `mcp_` 前缀挂载外部 MCP 服务；
  - Lua 5.4 扩展框架：独立的插件运行时与细粒度权限声明，支持通过安装独立插件扩展多样化领域与业务能力。

### 现代 Web 编程工作台

- **多标签工作区架构**：集成多会话切换、目录树浏览、Monaco 源代码编辑、xterm 终端控制台与 CDP 浏览器视图。
- **源代码管理（Source Control）**：内置 Git 变更审查面板，支持单文件及单行差异暂存（Staging）、撤销、提交、分支管理与图形化合并。
- **提示词模板系统**：输入框右上角提供模板管理抽屉，空会话与居中模式下展示快捷建议卡片（了解项目、审阅变更、规划测试等），支持 `/keyword` 实时检索与补全。
- **会话分支树（Turn Tree）**：支持从任一历史消息派生新分支，提供平移与缩放的全局分支视图。
- **外部引擎兼容**：输入框可直接挂载 Claude Code 或 Codex ACP 协议内核，在同一会话时间线内无缝协同。

### 终端交互与全屏转录

- **交互式流式 REPL**：多行编辑、剪贴板图片直接粘贴（`-c`）、Shell 透传（`!` 前缀）、快捷命令（`/` 前缀）。
- **全屏转录与折叠（Ctrl+O）**：全屏浏览历史上下文，折叠展开思考链与长命令输出，鼠标拖选支持 OSC 52 自动同步系统剪贴板。
- **交互式提问卡片**：圆角无边框布局，支持 1-9 数字键选定、空格切换、Tab 题间导航与 Other 自定义输入。
- **终端状态安全复原**：监听系统退出信号与异常中断，自动还原终端 Raw 模式、备用屏幕与键盘扩展协议。

### 跨会话长期记忆

- **双层存储**：`memory.db` 保存结构化事实（facts）与情景记忆（episodes）；`evicted_context.db` 存储上下文溢出历史。
- **FTS5 全文索引**：采用 unicode61 与 trigram 分词器，提供高精度中英文混合检索能力。
- **Markdown 文本双向同步**：记忆同步持久化为 `memory/files/` 目录下的纯文本 Markdown 文件，支持人工直接查阅与修正。
- **艾宾浩斯半衰期与联想召回**：每轮对话前依据语境提取关键词召回高频记忆，被召回记忆强度自动增强。

### 多聊天平台网关与常驻服务

- **主流通道接入**：官方 QQ 机器人、QQ 开放平台 OpenAPI、微信 iLink（长轮询与二维码登录）、OneBot v11、企业微信 Webhook。
- **统一生命周期管理**：`sai gateway start` 支持多通道并发启动与监管；网关通道工具支持将图片、文件与视频主动回推至聊天会话。

---

## 安装指南

### 系统要求

| 平台 | 环境依赖 |
| --- | --- |
| Linux (x86_64 / aarch64) | 推荐安装 `ripgrep`；沙盒隔离需要 `bubblewrap` |
| macOS (Apple Silicon / Intel) | 推荐安装 `ripgrep`；沙盒使用系统原生 `Seatbelt`；Web 工作台需要现代浏览器 |
| Windows (x86_64) | 推荐安装 `ripgrep`；Web 工作台支持 WebView2 或系统默认浏览器 |

### 从源码编译

开发环境需要：Rust stable、Node.js 22、pnpm。

```bash
# 1. 获取仓库代码
git clone https://github.com/jswysnemc/sai.git
cd sai

# 2. 构建前端静态资源 (Web 工作台)
cd web
pnpm install --frozen-lockfile
pnpm build
cd ..

# 3. 编译发布版可执行程序
cargo build --release --locked

# 4. 验证运行
./target/release/sai --version
```

Linux 系统安装编译依赖：

```bash
sudo apt-get install --yes \
  libasound2-dev \
  libwayland-dev \
  libxkbcommon-dev \
  pkg-config \
  ripgrep
```

### Arch Linux 打包

仓库提供 `scripts/package-arch.sh` 脚本，可快速生成 pacman 安装包：

```bash
cargo build --release --locked
bash scripts/package-arch.sh
sudo pacman -U ~/.cache/sai/packages/sai-<version>-1-x86_64.pkg.tar.zst
```

### 预编译二进制文件

每次推送到 `main` 分支均会触发自动化构建，可在 [GitHub Actions](https://github.com/jswysnemc/sai/actions) 下载各平台构件；正式发布版本请前往 [GitHub Releases](https://github.com/jswysnemc/sai/releases) 下载：

- `sai-linux-x86_64`
- `sai-windows-x86_64.exe`
- `sai-macos-arm64`

### Docker 容器部署

镜像托管于 GitHub Container Registry：

```bash
# 获取最新镜像
docker pull ghcr.io/jswysnemc/sai:latest

# 启动 Web 工作台服务
docker run --rm -it \
  -v "$HOME/.config/sai:/config/sai" \
  -v "$PWD:/workspace" \
  -p 4096:4096 \
  ghcr.io/jswysnemc/sai:latest web --port 4096 --no-open
```

---

## 快速上手

### 1. 初始化环境

初次运行会自动在标准配置路径生成骨架，也可以显式执行初始化：

```bash
sai init
```

### 2. 配置供应商与模型

在首次交互或启动 Web 界面时，系统会引导配置默认供应商与 API Key。也可以直接编辑配置文件（Linux `~/.config/sai/config.jsonc`）：

```jsonc
{
  "active_provider": "opencode",
  "providers": [
    {
      "id": "opencode",
      "display_name": "opencode Zen",
      "base_url": "https://opencode.ai/zen/v1",
      "protocol": "auto",
      "default_model": "big-pickle"
    }
  ]
}
```

密钥保存在同目录下的 `secrets.jsonc`，支持直接填写或引用系统环境变量：

```jsonc
{
  "api_keys": {
    "opencode": "$env:OPENCODE_API_KEY",
    "anthropic": "$env:ANTHROPIC_API_KEY"
  }
}
```

终端执行 `sai config` 或在 Web 界面右上角均可进入可视化设置中心。

### 3. 启动终端 REPL

```bash
sai
```

在 REPL 中：
- 输入文本直接发起多轮对话；
- 输入 `/plan` 切换至只读安全规划模式；
- 输入 `/sandbox` 检查当前沙盒保护策略；
- 输入 `/model` 调整当前模型、思考等级或为子智能体设置专属模型；
- 空输入状态下按 `?` 查看快捷键面板，按 `Ctrl+O` 进入全屏历史转录视图。

### 4. 单轮问答与快速任务

```bash
sai ask "请编写一段 Rust 实现的快速排序算法"
sai ask -c "分析剪贴板里的图片内容"          # 读取剪贴板图像
sai ask -w "Rust 最近版本发布的主要特性"       # 启用原生网页搜索
```

### 5. 启动 Web 编程工作台

```bash
sai web --port 4096
```

系统会自动调用默认浏览器打开 `http://localhost:4096`。若需局域网或公网访问，请先通过 `sai web-password set` 设定口令，再通过 `--host 0.0.0.0` 开放监听。

### 6. 终端命令接管（Shell Hook）

将未知命令交由 Sai 进行智能解析与修复建议：

```bash
sai zsh-init       # 或 bash-init / fish-init / powershell-init
exec $SHELL        # 重新载入当前 Shell
```

---

## CLI 命令参考

| 命令 | 说明 |
| --- | --- |
| `sai` | 启动交互式终端 REPL |
| `sai ask <message>` | 单轮对话任务；支持 `-c` 附加剪贴板图片，`-w` 启用网页搜索 |
| `sai web [--port N] [--host ADDR] [--no-open]` | 启动 Web 编程工作台 |
| `sai web-password set/clear/status` | 设置、清除或检查 Web 访问密码 |
| `sai init` | 初始化标准配置文件与状态目录 |
| `sai paths` | 打印当前配置、数据、缓存与状态目录的绝对路径 |
| `sai config` | 打开终端配置 TUI |
| `sai config validate` | 校验配置文件语法与有效性 |
| `sai models` | 交互式模型与思考强度选择器 |
| `sai providers [index]` | 查阅或切换当前生效的模型供应商 |
| `sai set thinking [level]` | 调整模型默认思考强度档位 |
| `sai fish-init` / `bash-init` / `zsh-init` / `powershell-init` | 输出或配置对应 Shell 的智能接管 Hook |
| `sai remove-shell-hook` | 安全卸载已配置的 Shell 接管 Hook |
| `sai history [--limit N] [--raw]` | 查阅历史会话记录 |
| `sai sessions list/new/switch/resume/delete/rename` | 会话管理工具集 |
| `sai resume [id]` | 恢复指定会话（省略参数进入交互式选择菜单） |
| `sai kb add/list/search/read/remove/reindex` | 本地知识库索引与检索管理 |
| `sai memory stats/reset/search/remember` | 长期记忆库查看、搜索与维护 |
| `sai skills list/show/enable/disable/remove/prune` | Skills 技能包管理 |
| `sai plugins list/info/init/pack/install/enable/disable` | Lua 插件安装、配置与权限授权 |
| `sai ps` | 查阅与管理后台运行的长任务进程 |
| `sai gateway start` | 一键并发启动配置文件中激活的所有网关通道 |
| `sai gateway qq-bot` / `weixin-server` / `onebot-server` | 独立启动特定平台的网关服务 |
| `sai weixin-login` | 启动微信 iLink 二维码扫码登录流程 |
| `sai compact` | 手动触发当前会话上下文归纳压缩 |
| `sai clear [--memory]` | 清理当前会话上下文或长期记忆 |

全局标志：`--lang en-US|zh-CN`、`--plan`、`--audited`、`--auto-audit`、`--yolo`、`--thinking LEVEL`、`-c`（剪贴板）、`-w`（网络搜索）。

---

## 系统架构

Sai 围绕统一的核心调度器（Runner）与 Agent 内核构建。各入口（REPL、CLI、Web、网关）将用户输入标准化后交由 Runner 派发，Agent 统一协同模型、沙盒、记忆库与工具系统。

![Sai 系统分层架构](pics/sai-architecture.svg)

### 核心技术栈

- **底层引擎**：Rust 2021 Edition、Tokio 异步运行时、rusqlite（SQLite WAL 与 FTS5 引擎）。
- **LLM 通讯**：reqwest + rustls，全双工 SSE 流式解码，三协议自适应转换层。
- **终端界面**：crossterm、termimad、syntect 代码高亮、KaTeX 终端渲染支持。
- **工作台后端**：axum 高并发 HTTP 与 WebSocket 服务，集成静态资源内嵌打包。
- **工作台前端**：React 19、Vite 8、TypeScript、TailwindCSS、Monaco Editor、xterm.js、Mermaid。

---

## 存储与目录布局

Sai 严格遵守各平台的标准目录规范（Linux 遵循 XDG，macOS 遵循 Application Support，Windows 遵循 Known Folders）。可通过 `sai paths` 查看具体物理路径。

### 配置目录（Config）

Linux: `~/.config/sai` | macOS: `~/Library/Application Support/sai` | Windows: `%APPDATA%\sai`

- `config.jsonc`：主配置文件（供应商列表、默认模型、Agent 偏好、网关设置等）
- `secrets.jsonc`：API 密钥与凭据文件，支持 `$env:VAR` 动态注入
- `mcp.jsonc`：MCP 外部服务连接描述
- `input-templates/`：用户自定义提示词模板（`chat/` 与 `image/` 分类）
- `skills/`：安装的全局与自定义 Skills 技能包
- `persona/`：Agent 人格定义、系统提示词与身份配置

### 状态目录（State）

Linux: `~/.local/state/sai` | macOS: `~/Library/Application Support/sai` | Windows: `%LOCALAPPDATA%\sai`

- `conversation.db`：会话轮次与消息流快照数据库
- `usage.json`：Token 用量消耗统计
- `loaded-tools.json`：跨轮次持久化的工具暴露状态
- `loaded-skills.json`：当前会话已激活技能清单
- `permission-audit.jsonl`：操作审批与沙盒调用审计流
- `plan.json`：当前会话未决或生效中的 Plan 规划快照

### 数据目录（Data）

Linux: `~/.local/share/sai` | macOS: `~/Library/Application Support/sai` | Windows: `%APPDATA%\sai`

- `persona/<name>/memory/memory.db`：长期记忆元数据与 FTS5 全文索引
- `persona/<name>/memory/files/`：落盘同步的 Markdown 记忆纯文本文件
- `persona/<name>/memory/evicted_context.db`：长上下文溢出历史归档
- `browser/profile/`：内建 CDP 浏览器隔离配置与存储

---

## 常见问题

**API Key 与鉴权信息会上传到云端吗？**
不会。所有 API 密钥严格保存在本地 `secrets.jsonc` 中。请求均由本地进程直接与大模型服务商通信；聊天网关仅用于透传对话消息。

**如何使用独立 Plan 模式？**
在终端 REPL 或 Web 工作台中输入 `/plan`，或由模型根据意图调用 `enter_plan_mode`。在此模式下，系统处于严格的只读沙盒环境，模型将专注于查阅工程现状并编写规范的实施规划，经人工审阅批准后方会切回原权限模式执行。

**沙盒环境在 Windows 上如何工作？**
Linux 基于 `bubblewrap`、macOS 基于 `Seatbelt` 实现内核级文件写隔离。Windows 目前支持高敏感目录（如 SSH、凭证、Git hooks）的主动拦截、路径合法性审计与人工操作逐次确认。

**子智能体（Subagents）会污染主工作区代码吗？**
不会。凡涉及文件写入与修改的子任务，系统均会自动在其专属的 `.sai-subagents` Git Worktree 隔离分支中运行。只有在子智能体验证成功且任务结束后，变更才会合并回工作区。

---

## 致谢与协议

本项目源自 [Miyu](https://github.com/SHORiN-KiWATA/Miyu)。感谢上游作者 [SHORiN-KiWATA](https://github.com/SHORiN-KiWATA) 开源的基础架构与核心理念。Sai 在此基础上持续维护、重构并演进。

本项目依据 [MIT](LICENSE) 许可证开源。
