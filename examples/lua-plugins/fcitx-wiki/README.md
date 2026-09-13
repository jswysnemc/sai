# Fcitx 5 官方文档查询

按问题或主题返回 Fcitx 5 规则，并可获取官方页面摘录。不依赖其他插件；输入法调查示例可将本包作为资料来源。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/fcitx-wiki
sai plugins pack examples/lua-plugins/fcitx-wiki --output ./fcitx-wiki-1.0.0.tar.gz
sai plugins install examples/lua-plugins/fcitx-wiki
sai plugins configure fcitx-wiki examples/lua-plugins/fcitx-wiki/settings.example.json
sai plugins enable fcitx-wiki --allow-http https://fcitx-im.org
sai --plan plugins call fcitx-wiki fcitx5_input_method_wiki_qurey '{"query":"Wayland input method","include_page_excerpt":false}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__fcitx-wiki__fcitx5_input_method_wiki_qurey`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

`include_page_excerpt=false` 可在零网络授权下读取包内规则；默认尝试获取摘录。请求失败时规则仍返回，摘录字段包含失败信息。可选参数包括 `topic`、`language`，保留原工具名称中的 `qurey` 拼写以维持包内契约。

本包没有业务设置，设置样本为 `{}`。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable fcitx-wiki --no-http
sai plugins disable fcitx-wiki
sai plugins remove fcitx-wiki
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。撤权后对应网络请求不能执行；能够返回离线或部分结果的工具会报告缺失资料。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info fcitx-wiki --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

