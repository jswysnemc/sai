# 已知 URL 正文读取

读取指定 HTTP(S) 地址，返回 Markdown、纯文本或 HTML。不执行搜索，不依赖其他插件，不需要模型。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/web-fetch
sai plugins pack examples/lua-plugins/web-fetch --output ./web-fetch-1.0.0.tar.gz
sai plugins install examples/lua-plugins/web-fetch
sai plugins configure web-fetch examples/lua-plugins/web-fetch/settings.example.json
sai plugins enable web-fetch --allow-http-read-any
sai --plan plugins call web-fetch web_fetch '{"url":"https://example.com","format":"text"}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__web-fetch__web_fetch`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

`url` 必填；`format` 默认 `markdown`，也支持 `text`、`html`。`timeout` 以秒为单位，上限 120；`max_chars` 默认 24000，上限 80000。任意来源读取授权包含本地 HTTP 服务，仅允许 GET/HEAD，不包含写入请求。返回内容始终作为不可信网页正文处理。

本包没有业务设置，设置样本为 `{}`。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable web-fetch --no-http
sai plugins disable web-fetch
sai plugins remove web-fetch
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。撤权后对应网络请求不能执行；能够返回离线或部分结果的工具会报告缺失资料。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info web-fetch --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

