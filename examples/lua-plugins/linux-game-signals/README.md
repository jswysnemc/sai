# Linux 游戏兼容性证据

从 Steam、ProtonDB、Can I Play on Linux 和 AreWeAntiCheatYet 汇总游戏兼容性证据、判断和置信度。本包只采集与整理资料，不调用模型。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/linux-game-signals
sai plugins pack examples/lua-plugins/linux-game-signals --output ./linux-game-signals-1.0.0.tar.gz
sai plugins install examples/lua-plugins/linux-game-signals
sai plugins configure linux-game-signals examples/lua-plugins/linux-game-signals/settings.example.json
sai plugins enable linux-game-signals --allow-http https://store.steampowered.com --allow-http https://www.protondb.com --allow-http https://caniplayonlinux.com --allow-http https://areweanticheatyet.com
sai --plan plugins call linux-game-signals gather_linux_game_compatibility_signals '{"game":"Cyberpunk 2077","issue":"multiplayer"}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__linux-game-signals__gather_linux_game_compatibility_signals`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

`game` 必填，`issue` 可选。各来源失败会记录在证据中，已有资料仍可返回；撤销某个来源后，不应把缺失资料理解为兼容性结论。本包不依赖 `protondb` 插件，使用自身 HTTP 采集模块；`linux-game-investigation` 会通过完整工具名调用本包。

本包没有业务设置，设置样本为 `{}`。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable linux-game-signals --no-http
sai plugins disable linux-game-signals
sai plugins remove linux-game-signals
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。撤权后对应网络请求不能执行；能够返回离线或部分结果的工具会报告缺失资料。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info linux-game-signals --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

