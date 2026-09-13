# 答复通知示例

此包为 TUI 和 Web 的答复完成、中断、失败事件生成通知数据。宿主负责系统或浏览器投递；插件无需访问消息正文、文件、模型或其他插件。单次 CLI 调用和子任务不触发这个交互面事件。

```sh
sai plugins check ./examples/lua-plugins/reply-notification
sai plugins pack ./examples/lua-plugins/reply-notification --output ./reply-notification.tar.gz
sai plugins install ./examples/lua-plugins/reply-notification
sai plugins configure reply-notification ./examples/lua-plugins/reply-notification/settings.example.json
sai plugins enable reply-notification --allow-notifications
sai --plan plugins run reply-notification preview '{"surface":"tui","status":"completed","locale":"zh-CN"}'
```

`preview` 只返回通知数据，不显示通知或播放声音。正式交互面需要 `notifications` 授权；它不授予 `sai.notify.send` 的主动投递权限。通知可用性仍取决于操作系统、浏览器权限及终端环境，见[通知纯回调](../../../design/lua-plugins/api.md#通知纯回调)。

设置只有两个布尔值：`enabled` 控制桌面通知，`sound` 控制声音，缺省均为 `true`。插件启停开关与这两个设置分别保存。旧主配置 `notification` 字段不再参与计算；未安装或未授权时不会自动采用内置通知策略。

```sh
sai plugins install ./new-reply-notification --replace
sai plugins enable reply-notification --no-notifications
sai plugins disable reply-notification
sai plugins remove reply-notification
```

更新保留设置与授权；普通重新启用不会恢复撤权。此包不保存业务数据，卸载只移除源码，独立设置仍留在 `plugins.jsonc`。需要恢复通知时重新安装并明确授权。
