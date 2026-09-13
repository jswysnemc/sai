# 在线 Linux 手册

搜索 Arch Linux 手册，并从 Arch 手册站点或 man7 读取正文。无需本机安装目标软件，不依赖其他插件。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/online-man
sai plugins pack examples/lua-plugins/online-man --output ./online-man-1.0.0.tar.gz
sai plugins install examples/lua-plugins/online-man
sai plugins configure online-man examples/lua-plugins/online-man/settings.example.json
sai plugins enable online-man --allow-http https://man.archlinux.org --allow-http https://man7.org
sai --plan plugins call online-man online_man_get_page '{"name":"printf","section":"3","source":"arch"}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__online-man__online_man_get_page`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

`online_man_search` 接收 `query`，可选 `section`、`language`、`limit`。`online_man_get_page` 接收 `name`，可选 `section`、`source`、`language` 和 `max_chars`；`source` 为 `auto`、`arch`、`man7`。只选择一个来源时可仅授予对应 HTTP 来源。

本包没有业务设置，设置样本为 `{}`。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable online-man --no-http
sai plugins disable online-man
sai plugins remove online-man
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。撤权后对应网络请求不能执行；能够返回离线或部分结果的工具会报告缺失资料。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info online-man --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

