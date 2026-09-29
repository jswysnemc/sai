# 会话恢复与工作区

`sai resume`、`sai sessions resume` 和 REPL 中的 `/resume` 默认只显示当前工作目录的会话。工作区使用规范化路径标识，符号链接路径与原路径共用同一份会话目录。

## 使用方式

- Tab：切换当前工作区和全部工作区，保留搜索内容。
- 输入文字：按标题、会话 ID 或工作区路径过滤。
- 上下键、PageUp/PageDown、Home/End：移动选择。
- Enter：恢复选中会话。
- Esc：取消，保持当前会话与目录。

全部视图按工作区路径分组，组内按更新时间排列。列表展示标题、相对时间、会话 ID 和所在工作区的活动标记；底部显示选中会话的完整目录、更新时间及在线实例类型。

```sh
sai resume --all
sai resume <session-id>
sai resume <session-id> --workspace /path/to/project
sai resume --workspace /path/to/project
```

按 ID 恢复优先匹配当前工作区；当前工作区没有该 ID 时查找其他工作区。多个外部工作区存在同名 ID 时，必须通过 `--workspace` 明确目录，或在全量选择器中选择目标。

## 恢复边界

选择结果包含工作区身份、会话身份和目录，不能只用会话 ID 定位。恢复先验证目录和索引身份，再在目标目录打开会话并重建 Agent、权限配置和工具注册表。准备成功后才切换目标工作区的活动指针；准备失败时恢复原进程目录并保留原 Agent。

跨工作区恢复会改变当前 Sai 进程的 CWD，后续 Shell 命令、文件工具、文件补全和后台工具发现均使用目标目录。启动 Sai 的父 Shell 目录不受子进程目录变化影响。

旧工具预热任务固定使用启动时捕获的目录；恢复会替换预热任务，旧结果不能覆盖新会话工具。恢复会重新读取完整会话历史，旧会话的输入草稿和排队消息不会带入目标会话。

## 旧记录兼容

新建或打开会话时，在工作区会话目录保存 `workspace.json`，记录规范化路径。文件通过临时文件替换，避免其他进程读到部分内容。列出会话不补建会话数据。

旧记录优先通过当前 CWD 或既有 Web 工作区注册表补全路径。若只剩不可逆的目录哈希，TUI 会打开统一表单，请求输入原目录；系统验证目录哈希与目标索引一致后才恢复。成功后补录路径。也可使用 `--workspace` 指定目录。不存在或身份不符的目录不能用于恢复。

## 模块

```text
src/state/sessions/
  workspace_metadata.rs        路径保存与旧记录兼容
  resume_catalog.rs            目录快照、精确目标和身份验证
src/cli/session_picker/
  model.rs                     范围、过滤与选择状态
  view.rs                      分组列表及元数据
  workspace_prompt.rs          旧工作区目录定位表单
  mod.rs                       按键循环与终端恢复
src/cli/session_resume.rs       CLI 恢复和进程目录事务
src/cli/repl/session_resume.rs  Agent 与工具环境重建
```

## 回归覆盖

单元测试覆盖工作区过滤、分组、同名会话、路径缺失、Web 路径兼容、符号链接和后台目录隔离。真实终端测试覆盖默认视图、Tab 切换、CLI `--all`、显式目录、取消、目录删除、数据库损坏、旧目录补录、历史恢复及真实模型文件工具在目标目录执行。

验证命令：

```sh
cargo test --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --locked --bin sai
uv run scripts/tui_regression.py
cd web
npm test
```
