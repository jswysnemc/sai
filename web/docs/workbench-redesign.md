# Sai Web 工作台改造与验收

本次改造将 Sai Web 调整为以项目会话为主、按需打开文件、变更审阅和终端的开发工作台。统一浅色与石墨主题、导航、输入区、设置页和移动端布局，同时修复文件选择、终端生命周期和跨项目切换中的交互问题。

验收日期：2026-09-07。

## 对齐依据

参考 [Codex app 功能说明](https://developers.openai.com/codex/app/features)、[Cursor Agents Window 文档](https://cursor.com/docs/agent/agents-window.md)及其[官方界面截图](https://cursor.com/docs-static/images/agent/file-agents-window-final.png)，采用项目会话导航、居中任务输入、紧凑工具栏、按需审阅和终端等结构。

Codex app、ChatGPT 桌面客户端和 Cursor 是不同产品。本次实现保留 Sai 品牌与现有后端能力，对齐上述开发工作台的视觉层级和操作方式；尚未完成与指定客户端版本的逐屏像素比对，不能将结果视为已经验证的一比一复刻。

## 改造评估

| 区域 | 改造前的问题 | 当前实现 |
| --- | --- | --- |
| 视觉基础 | 大面积灰绿色表面、控件强调方式不统一 | 白色与中性石墨表面，统一边框、悬停、选中和焦点状态，状态色用于运行与变更信息 |
| 工作台布局 | 空白文件面板占用新会话空间，项目信息重复出现 | 默认聚焦对话，文件与审阅按需展开，项目和分支集中于状态栏 |
| 项目导航 | 常用操作分散，项目与会话层级不明显 | 项目分组、折叠、项目内新建任务，以及固定的搜索、定时任务和技能入口 |
| 新会话 | 空状态缺少明确起点 | 紧凑问候、居中输入区、三个任务建议；建议仅填入草稿，由用户发送 |
| 输入区 | 模型、模式与上下文入口挤在同一层 | 模型与附件位于输入框操作行，权限和上下文用量位于辅助行 |
| 对话内容 | 上下文和工具过程占据过多注意力 | 上下文默认单行摘要，工具过程折叠，消息操作使用统一悬停与键盘焦点样式 |
| 文件工作区 | 标签、面板入口和内容高度不协调，切回编辑器可能选错文件 | 统一标签导航和添加面板菜单，按请求路径选择文件，支持键盘移动和收起后恢复 |
| 搜索 | 工作台操作、会话和文件入口割裂 | 同一搜索弹层支持命令、跨项目会话及当前项目文件，支持方向键与回车 |
| 设置 | 导航与表单缺少一致层级，小屏分类容易离开可视范围 | 统一导航、表单和保存栏，移动端自动显示当前分类并清除不可见搜索条件 |
| 小屏交互 | 761–767 px 存在断点空档，抽屉与桌面布局切换不一致 | 使用与 Tailwind `md` 对齐的 `48rem` 边界；抽屉限制焦点、屏蔽背景交互并支持 Escape 返回 |

界面字体采用 Inter 与现有中文字体，代码继续使用 Fira Code。工具栏高 `3rem`，状态栏高 `1.75rem`，正文与控件字体使用相对单位。Monaco 的字体参数遵循其像素单位接口。

## 模块职责

关键文件按功能与职责拆分，完整实现仍位于现有 `features/` 和 `shared/` 目录中：

```text
src/
├─ shared/
│  ├─ styles/tokens/
│  │  ├─ neutral-themes.css          # 浅色、石墨及系统主题
│  │  └─ workbench.css               # 工作台表面与尺寸变量
│  └─ ui/
│     ├─ button/                    # 统一按钮样式与尺寸
│     └─ menu/                      # 可复用操作菜单
└─ features/
   ├─ sessions/
   │  ├─ sidebar-project-group.tsx   # 项目分组与操作入口
   │  └─ use-session-actions.ts      # 项目和会话导航
   ├─ chat/
   │  ├─ chat-empty-state.tsx         # 新任务入口与草稿建议
   │  ├─ chat-conversation.tsx        # 对话内容组合
   │  ├─ chat-session-dialogs.tsx     # 会话相关弹窗
   │  ├─ chat-error-notices.ts        # 聊天错误提示
   │  └─ chat-composer/               # 模型控件、上下文辅助行与类型
   ├─ search/
   │  ├─ global-search-dialog.tsx     # 搜索交互与结果执行
   │  └─ search-file-results.ts       # 文件匹配与排序
   ├─ workspace/
   │  ├─ workspace-layout.tsx         # 工作台布局组合
   │  ├─ workbench-toolbar.tsx        # 当前会话和工作区操作
   │  ├─ workbench-status-bar.tsx     # 项目、分支和运行状态
   │  ├─ workbench-shortcuts.ts       # 统一快捷键解析与执行
   │  ├─ use-mobile-sidebar-dialog.ts # 移动端侧栏焦点管理
   │  ├─ workspace-panel-selection.ts # 面板和文件选择规则
   │  ├─ monaco-typescript.ts         # 单文件模式的语言诊断配置
   │  ├─ unsaved-editor-changes.ts    # 未保存文件登记
   │  └─ workspace-*.css             # 标签、编辑器、Git 和空状态样式
   ├─ workspaces/
   │  └─ workspace-switch-confirmation.ts # 未保存文件与终端切换确认
   ├─ terminal/
   │  └─ terminal-disposal.ts         # 终端初始化与销毁时序
   └─ settings/
      ├─ settings-navigation.css     # 设置导航布局
      └─ shell/settings-nav.tsx      # 分类、搜索和小屏当前项定位
```

修改过的源文件均低于 750 行；聊天内容、输入控件、弹窗、错误提示和工作区样式已分别拆分。

## 交互修复

- **文件选择**：打开多个文件后进入 Git 面板，再选择某个文件标签时，优先恢复指定文件，不再跳到第一个文件。
- **面板保持**：收起工作区保留挂载状态，再次展开时恢复文件内容与选择状态。
- **跨项目导航**：切换项目后重新载入工作台，重建文件、Git 和终端上下文；未保存文件先经过统一确认弹窗，取消时不请求后端切换，保留编辑内容。终端占用继续使用已有确认流程。
- **文件搜索排序**：匹配程度相同时优先显示较浅路径，使根目录 README 排在内部工作树副本之前。
- **终端快捷键**：终端取得焦点时也能打开搜索、收起终端；长按组合键不重复执行工作台操作或写入终端。
- **终端销毁**：Xterm 初始化队列完成后再销毁实例，避免 StrictMode 清理或快速重开触发 `Viewport.syncScrollArea` 的 `dimensions` 异常。
- **编辑器诊断**：当前 Monaco 仅加载打开文件，未接入完整项目类型上下文。关闭不可靠的 TypeScript/JavaScript 语义诊断与建议诊断，保留语法检查；JSON 校验继续生效。
- **小屏弹层**：侧栏关闭后恢复触发按钮焦点；工作概览保持在屏幕内，Escape 关闭后恢复工具栏焦点。
- **文件标签**：标签按内容分配宽度，同名文件显示最短区分目录；标签溢出时可通过路径菜单选择，活动标签自动进入可视范围。
- **差异审阅**：提供旧、新行号、连续语法着色、字符差异、统一及并排布局、换行开关、文件筛选、折叠和 F7 变更跳转。窄栏自动使用统一视图，保留宽屏布局偏好。
- **大补丁**：汇总补丁截断后仍可单独读取文件；单文件补丁也截断时明确提示并禁用行级操作。仅实际载入的上下文可以展开，补丁缺失区域显示省略提示。
- **窗口缩放**：桌面全屏审阅缩到移动端后继续显示工作区；返回聊天清除全屏状态。文件导航按工具栏当前高度判断可见区域，避免统计被遮挡的上一文件。

常用快捷键：

| 快捷键 | 操作 |
| --- | --- |
| Ctrl/Cmd + K 或 Ctrl/Cmd + Shift + P | 搜索命令、会话与文件 |
| Ctrl/Cmd + Shift + O | 新建任务 |
| Ctrl/Cmd + B | 切换会话侧栏 |
| Ctrl/Cmd + J | 切换终端 |
| Ctrl/Cmd + Shift + E | 浏览文件 |
| Ctrl/Cmd + Shift + G | 审阅变更 |
| Ctrl/Cmd + I | 聚焦消息输入区 |

快捷键解析会避开输入法组合过程；存在对话框或菜单时，工作台快捷键交还控制权。

## 验证

前端沿用仓库 CI 使用的 npm 和 `package-lock.json`。新增 Inter 字体依赖已经写入该锁文件。

在 `web/` 目录运行：

```sh
npm test -- --reporter=dot
npm run build
```

在仓库根目录运行：

```sh
git diff --check
```

| 检查 | 结果 |
| --- | --- |
| 自动化测试 | 199 个测试文件、957 项测试通过 |
| TypeScript 与生产构建 | 通过；构建命令包含 `tsc -b` |
| 差异空白检查 | 通过 |
| 响应式布局 | 检查 320、390、761、767、768、1024、1440 px；页面与主要控件无横向溢出 |
| 主题与设置 | 浅色、石墨、小屏当前分类可见性、中英文切换通过 |
| 搜索与标签 | 文件搜索、多文件选择、方向键、Home/End、收起与恢复通过 |
| 终端 | 连续重开 3 次，终端内打开搜索和关闭面板通过，未产生新的页面异常 |
| Monaco | 有效 TSX/JavaScript 文件无依赖缺失误报；错误语法仍报告诊断；错误 JSON 继续报告诊断 |
| 项目切换 | Sai 与隔离测试项目之间往返，旧文件标签和搜索结果未沿用；完成后恢复 Sai |
| 未保存编辑 | 取消切换保留原项目和编辑内容；确认后切换；还原编辑后不再重复提示 |
| 生产页面异常 | 本轮浏览器验收未记录到页面异常 |

会话轮次预览和运行状态接口通过 Rust 全量测试：2,544 项通过、8 项跳过。修改过的 Rust 文件通过格式检查；全仓 `cargo fmt --check` 仍报告既有文件的格式差异。

浏览器验证使用 Chromium、生产预览与独立后端配置目录。对话内容通过隔离测试数据覆盖时间线接口，检查完成后移除覆盖；没有通过真实模型请求生成测试对话。验证未覆盖真实供应商流式响应、真实移动设备软键盘、Safari 和 Firefox。

## 界面截图

对话、编辑器及移动端对话截图使用测试会话内容。其中的失败提示是预设场景，用于检查错误与重试入口。截图展示真实页面渲染，文件编辑区读取当前仓库文件。

| 场景 | 截图 |
| --- | --- |
| 改造前工作台 | [workbench-before.png](screenshots/workbench-before.png) |
| 浅色新任务页 | [workbench-light.png](screenshots/workbench-light.png) |
| 浅色设置页 | [settings-light.png](screenshots/settings-light.png) |
| 石墨主题对话 | [conversation-dark.png](screenshots/conversation-dark.png) |
| 命令与文件搜索 | [command-search.png](screenshots/command-search.png) |
| 对话与代码编辑 | [editor-dark.png](screenshots/editor-dark.png) |
| 移动端对话与错误状态 | [conversation-mobile.png](screenshots/conversation-mobile.png) |

## 当前边界

- 生产构建仍提示部分资源超过 500 kB，包含 Monaco、Mermaid 和应用主包。本次完成视觉与交互验收，未完成加载性能专项优化。
- Monaco 尚未读取完整 `tsconfig`、依赖声明或语言服务器，不能提供项目级类型检查；应以项目自身构建和检查命令为准。
- 跨项目切换使用整页重新载入，打开的文件标签不会跨项目保留；当前没有跨重载编辑草稿恢复功能。
- 保留 Sai 现有模型、工具、权限、后台任务和网关能力；没有新增 Codex 或 Cursor 专有云端服务，也没有对其功能完整性作等同承诺。
