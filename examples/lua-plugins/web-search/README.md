# 多供应商网页搜索

通过 TinyFish、Tavily、Firecrawl、AnySearch、SearXNG 或 DuckDuckGo 查询并返回统一 Markdown。无需另一个搜索插件；本包不读取旧主配置中的搜索凭据。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/web-search
sai plugins pack examples/lua-plugins/web-search --output ./web-search-1.0.0.tar.gz
sai plugins install examples/lua-plugins/web-search
sai plugins configure web-search examples/lua-plugins/web-search/settings.example.json
sai plugins enable web-search --allow-http https://html.duckduckgo.com
sai --plan plugins call web-search web_search '{"query":"Rust ownership","provider":"duckduckgo","max_results":5}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__web-search__web_search`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

示例仅使用不需要密钥的 DuckDuckGo。`default_provider=auto` 按 TinyFish、Tavily、Firecrawl、AnySearch、SearXNG、DuckDuckGo 顺序尝试已启用服务；单个服务失败会继续回退。结果数量为 1–10。完整设置及默认值见 [config.lua](config.lua)。

密钥可以通过 `<provider>_api_keys` 数组提供，或通过 `$env:VARIABLE` 引用；授权后也会尝试对应的 `TINYFISH_API_KEY`、`TAVILY_API_KEY`、`FIRECRAWL_API_KEY`、`ANYSEARCH_API_KEY`。例如 Tavily 需要 `--allow-env TAVILY_API_KEY --allow-http https://api.tavily.com --allow-http-read-only-post https://api.tavily.com/search`。读取环境发生在调用时，初始化和保存配置不展开凭据；未授权变量不可读取。

Firecrawl 和 AnySearch 的查询 POST 端点分别为 `https://api.firecrawl.dev/v2/search`、`https://api.anysearch.com/v1/search`，需要同时授予来源和精确只读 POST 端点。TinyFish 使用 `https://api.search.tinyfish.ai`。自建 SearXNG 或更改任一服务地址时，须修改自己的包清单声明、保存独立地址设置并显式授权；设置不会自动扩大网络权限。

设置样本不包含密钥，私有配置文件不应放入分发包。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable web-search --no-http --no-env
sai plugins disable web-search
sai plugins remove web-search
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。撤权后对应网络请求不能执行；能够返回离线或部分结果的工具会报告缺失资料。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info web-search --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

