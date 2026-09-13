# DeepSeek 服务状态

读取官方状态页面，返回服务、组件、可用率和近期事件。查询不需要模型，也不依赖其他插件。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/deepseek-status
sai plugins pack examples/lua-plugins/deepseek-status --output ./deepseek-status-1.0.0.tar.gz
sai plugins install examples/lua-plugins/deepseek-status
sai plugins configure deepseek-status examples/lua-plugins/deepseek-status/settings.example.json
sai plugins enable deepseek-status --allow-http https://status.deepseek.com
sai --plan plugins call deepseek-status query_deepseek_status '{"include_incidents":true,"max_incidents":5}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__deepseek-status__query_deepseek_status`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

`include_incidents` 默认开启，`max_incidents` 默认 5，实际范围为 1–20。上游页面结构变化可能导致解析失败；此结果表示官方状态页内容，不代替实际模型请求。

本包没有业务设置，设置样本为 `{}`。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable deepseek-status --no-http
sai plugins disable deepseek-status
sai plugins remove deepseek-status
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。撤权后对应网络请求不能执行；能够返回离线或部分结果的工具会报告缺失资料。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info deepseek-status --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

