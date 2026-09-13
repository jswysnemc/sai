# 会话待办示例

此包提供 `lua__todo__todo`，包含清单增删改查、顺序推进、完成归档和工具循环提醒。它使用公共会话存储，没有其他插件依赖；未安装、禁用或未授予存储能力时，Web 待办视图为空。

## 安装与使用

```sh
sai plugins check ./examples/lua-plugins/todo
sai plugins pack ./examples/lua-plugins/todo --output ./todo.tar.gz
sai plugins install ./examples/lua-plugins/todo
sai plugins configure todo ./examples/lua-plugins/todo/settings.example.json
sai plugins enable todo --allow-session-storage --allow-reply-policy
sai --yolo plugins call --session default todo todo '{"action":"add","text":"检查配置"}'
sai --plan plugins run --session default todo snapshot
```

`--session` 选择当前工作区内的会话 ID，与 Agent、TUI 和 Web 共用该会话的存储目录；非法或不存在的会话会报错。省略时使用当前工作区专属的命令作用域，不会修改真实会话计划。TUI 中也可通过 `/plugins run todo snapshot` 查询当前会话。

`todo` 工具声明写入，包括 `list` 动作；计划模式可用只读 `snapshot` 命令查询。`language` 只接受 `en` 或 `zh`。`session_storage` 用于条目与历史，`reply_policy` 仅用于连续三个工具轮未更新计划时的提醒，两项分别授权。

## 显式导入旧记录

宿主不再按插件 ID 自动读取 `todos.json`、`todos.history.json` 或 `todos.plugin.json`。这些文件保持原样。将旧完整快照复制到当前工作区的 `.sai/todo-import/snapshot.json` 后执行：

```sh
sai plugins enable todo --allow-read-path .sai/todo-import
sai --yolo plugins run --session default todo import '{"path":".sai/todo-import/snapshot.json"}'
```

也可传入 `{"state":{"version":1,"items":[],"history":[]}}`，无需文件读取授权。只有旧分离文件时，明确组装 `version: 0`、`items` 与 `history`；每个条目需保留 `id/text/status/created_at/updated_at`，每批历史需保留 `archived_at/items`。

导入校验版本、重复 ID、状态和 256 KiB 上限，并以比较交换提交。现有条目或历史非空时拒绝覆盖；空视图初始化不阻止导入。来源文件不会被改写或删除，未授权路径与计划模式均不能导入。

## 更新与数据保留

```sh
sai plugins install ./new-todo --replace
sai plugins enable todo --no-session-storage --no-reply-policy
sai plugins disable todo
sai plugins remove todo
```

更新、禁用和卸载保留已保存的公共计划；重新安装仍需重新授权。清空对话会按公共会话存储规则清除当前条目与历史，旧会话快照保留。整体清空会话数据或删除会话还会删除会话目录内的旧文件；需要保留的导入备份应放在会话目录外。需要长期备份时先保存 `snapshot` 返回的 `items/history`，并补上 `version: 1`。

源码回退不会回退数据。当前状态版本为 1，支持导入版本 0 和 1；损坏记录明确报错，不会当作空计划覆盖。详细规则见[待办设计](../../../design/lua-plugins/todo.md)。
