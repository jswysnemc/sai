# 闹钟示例

此包提供 `lua__alarm__set_alarm`、`lua__alarm__list_alarms` 和 `lua__alarm__cancel_alarm`。提醒规则在 Lua 中执行，宿主提供持久调度、进程取消和声音投递。没有其他插件依赖；未安装时核心不会自动创建提醒。

## 安装与运行

```sh
sai plugins check ./examples/lua-plugins/alarm
sai plugins pack ./examples/lua-plugins/alarm --output ./alarm.tar.gz
sai plugins install ./examples/lua-plugins/alarm
sai plugins configure alarm ./examples/lua-plugins/alarm/settings.example.json
sai plugins enable alarm --allow-schedule --allow-notify
sai --yolo plugins call alarm set_alarm '{"time":"10m","label":"休息"}'
sai --plan plugins call alarm list_alarms '{}'
sai --yolo plugins call alarm cancel_alarm '{"id":"返回的任务标识"}'
```

`settings.example.json` 是空对象。时间支持时长和本地时刻，原有时间、标签、音频大小与格式校验继续生效。默认声音无需读取用户文件。自选 WAV/MP3 还需明确读取授权，例如 `--allow-read-path .`；超出清单的音频目录必须先修改清单再替换安装。旧 `audio_paths` 设置不再派生权限。

`deliver` 是调度器使用的写入命令。实际声音和桌面投递依赖宿主平台与可用音频设备；远端服务器上的通知不会自动转发到浏览器。支持边界见[通知接口](../../../design/lua-plugins/notification-api.md)和[调度接口](../../../design/lua-plugins/scheduler-api.md)。

## 任务与卸载

```sh
sai plugins jobs alarm list
sai plugins jobs alarm cancel TASK_ID
sai plugins enable alarm --no-schedule --no-notify
sai plugins disable alarm
sai plugins remove alarm
```

管理命令可以查看和取消禁用、卸载后保留的任务。到期执行会重新核对安装、启用、源码修订和授权；卸载不会把任务转交给其他包，也不会自动重新投递。

旧 `alarms.json` 保持原样。只有显式的 `plugins jobs alarm` 管理入口能查看、取消旧任务；普通 Lua 调度不能凭 `alarm` 这个 ID 读取它们。旧任务禁止恢复或重放，存量工作进程仍须通过身份和当前授权检查。

更新使用 `sai plugins install ./new-alarm --replace`，不会恢复撤销的权限。源码可用旧包回退；任务状态不会随源码回退。任务记录属于应用状态目录，清理与取消分别处理。
