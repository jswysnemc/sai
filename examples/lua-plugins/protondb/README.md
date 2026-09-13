# ProtonDB 查询

按 Steam App ID 或游戏名称查询兼容性评级和有数量限制的用户评论。不依赖其他插件，也不调用模型。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/protondb
sai plugins pack examples/lua-plugins/protondb --output ./protondb-1.0.0.tar.gz
sai plugins install examples/lua-plugins/protondb
sai plugins configure protondb examples/lua-plugins/protondb/settings.example.json
sai plugins enable protondb --allow-http https://www.protondb.com --allow-http https://94he6yatei-dsn.algolia.net
sai --plan plugins call protondb protondb_query '{"query":"1245620","max_reports":5}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__protondb__protondb_query`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

`query` 必填，既可为数字 App ID，也可为名称。`max_reports` 默认 10，最多 40；评级获取失败会返回错误，评论不可用时仍保留已取得的游戏资料和评级。Algolia 来源用于游戏匹配，ProtonDB 来源用于评级与评论。

本包没有业务设置，设置样本为 `{}`。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable protondb --no-http
sai plugins disable protondb
sai plugins remove protondb
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。撤权后对应网络请求不能执行；能够返回离线或部分结果的工具会报告缺失资料。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info protondb --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

