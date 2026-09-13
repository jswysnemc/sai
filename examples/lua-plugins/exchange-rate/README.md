# 汇率查询

查询基础币种到目标币种的汇率，支持 ISO 代码与常见中文币种名称。可使用配置密钥的服务，或无密钥的免费来源。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/exchange-rate
sai plugins pack examples/lua-plugins/exchange-rate --output ./exchange-rate-1.0.0.tar.gz
sai plugins install examples/lua-plugins/exchange-rate
sai plugins configure exchange-rate examples/lua-plugins/exchange-rate/settings.example.json
sai plugins enable exchange-rate --allow-http https://open.er-api.com
sai --plan plugins call exchange-rate get_exchange_rate '{"base":"USD","target":"CNY"}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__exchange-rate__get_exchange_rate`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

`api_key` 默认空，`free_fallback_enabled` 默认 `true`。密钥须写入自己的设置文件；本包不读取旧主配置，也不展开环境变量引用。启用付费来源另需 `--allow-http https://v6.exchangerate-api.com`。服务返回有效 JSON 却没有成功汇率时才尝试免费回退；HTTP、解析或权限错误直接返回。禁用免费回退后必须配置密钥与对应来源。

设置样本不包含密钥，私有配置文件不应放入分发包。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable exchange-rate --no-http
sai plugins disable exchange-rate
sai plugins remove exchange-rate
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。撤权后对应网络请求不能执行；能够返回离线或部分结果的工具会报告缺失资料。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info exchange-rate --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

