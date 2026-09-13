# Linux 游戏兼容性调查

组织模型调查，结合兼容性信号、可选查询资料与本地事实形成报告。保留模型用量、工具统计和有界进度。

## 安装、配置与运行

从仓库根目录执行；独立分发后将路径替换为自己的包目录。

```sh
sai plugins check examples/lua-plugins/linux-game-investigation
sai plugins pack examples/lua-plugins/linux-game-investigation --output ./linux-game-investigation-1.0.0.tar.gz
sai plugins install examples/lua-plugins/linux-game-investigation
sai plugins configure linux-game-investigation examples/lua-plugins/linux-game-investigation/settings.example.json
sai plugins enable linux-game-investigation --allow-model --allow-tool lua__linux-game-signals__gather_linux_game_compatibility_signals
sai --plan plugins call linux-game-investigation linux_game_compatibility '{"game":"Cyberpunk 2077","issue":"multiplayer"}'
```

安装默认禁用且没有授权。模型使用的工具名称为 `lua__linux-game-investigation__linux_game_compatibility`，其他入口也使用同样的包前缀。安装不会修改 Agent 白名单。配置样本不包含真实密钥或用户数据；旧主配置选项不再注入本包。

## 参数、权限与依赖

必需依赖是 `linux-game-signals`：先普通安装、启用并授予所需 HTTP 来源，再授权本包调用其完整工具名。依赖不可用时，在发起模型请求前返回明确错误。可选依赖为 `protondb`、`web-search`、`web-fetch`、`knowledge-base`；每项都按清单与用户授权交集进入目录。

`game` 必填，`issue` 可选。`max_tool_steps=0` 由宿主预算限制，正数增加业务调用上限；`progress_mode` 为 `hidden`、`summary` 或 `full`。模型使用当前宿主选择，无供应商凭据注入。最终报告保留证据缺失与不确定性，不将来源不可用解释为兼容或不兼容。

配置仅保存业务设置，不调整能力声明或恢复撤权。具体字段见 [settings.example.json](settings.example.json) 和包内实现，公共权限说明见[示例索引](../README.md)。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins disable linux-game-investigation
sai plugins remove linux-game-investigation
```

使用 `sai plugins enable linux-game-investigation` 的 `--no-http`、`--no-model`、`--no-tools`、`--no-processes`、`--no-file-write` 等对应选项撤销能力；只指定需要撤销的项，其余保持原值。普通重新启用不会恢复权限。替换安装保留设置与现有授权，新增声明须明确授权；已有会话通过 `/plugins reload` 使用新快照。

不保存额外业务数据；报告和用量记录留在所属会话。 源码回退使用保留的旧发行包，与数据删除分开处理。

找不到工具时检查安装、启用状态和完整名称；权限错误检查 `sai plugins info linux-game-investigation --json`。缺少模型、系统工具、网络来源或可选插件时按错误信息修正对应依赖。验证使用隔离路径、本地服务和固定样本，不代表已经实测所有平台或外部供应商。

