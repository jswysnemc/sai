# 当前天气查询

本示例通过 wttr.in 查询当前天气，支持城市名、机场代码和服务端自动定位。使用公共 HTTP 与文本 API，不依赖其他插件，不保存用户数据。

## 安装、授权与运行

以下命令从 sai 仓库根目录执行；独立分发后将源码路径替换为解压目录：

```sh
sai plugins check examples/lua-plugins/weather
sai plugins pack examples/lua-plugins/weather --output ./weather-1.0.0.tar.gz
sai plugins install examples/lua-plugins/weather
sai plugins configure weather examples/lua-plugins/weather/settings.example.json
sai plugins enable weather --allow-http https://wttr.in
sai --plan plugins call weather get_weather '{"location":"Beijing"}'
```

`check` 应报告一个工具和零个命令。运行结果以 `current weather(condition,temperature,wind,location): ` 开头。无需配置模型；实际查询需要能够访问 wttr.in，支持 sai HTTP 宿主可用的平台。

最小授权只包含 `https://wttr.in`。普通 `enable weather` 不授权网络；本包不声明任意来源读取、POST 写入或其他系统能力。模型工具名称为 `lua__weather__get_weather`，使用 Agent 白名单时显式选择；旧 `plugins.weather` 开关和短名称不再自动提供天气工具。

## 参数与服务边界

`location` 是可选字符串，空值或空白使用服务端自动定位，非空值先去除两端空白再进行 URL 编码。未知参数和非字符串地点会由工具 Schema 拒绝。

本包没有业务设置，示例配置为 `{}`。服务来源固定在清单和 `query.lua` 中；派生版本若要改用其他服务，须一起修改实现、清单与用户授权，单独保存设置不能扩大权限。

请求超时为 30 秒，响应最多 65536 字节。HTTP 非成功状态、空白响应、超限和传输错误会返回错误。地点编码、错误状态和正文边界由固定响应样本验证，测试不要求连接外网。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins enable weather --no-http
sai --plan plugins call weather get_weather '{"location":"Beijing"}'
sai plugins disable weather
sai plugins remove weather
```

撤权后的调用应失败，重新启用不会恢复 HTTP 权限。重新授权时再次明确提供 `--allow-http https://wttr.in`。替换安装保留设置和现有授权；已有会话执行 `/plugins reload` 后使用新快照。

找不到工具时检查安装、启用状态和完整名称；网络拒绝时查看 `sai plugins info weather --json` 的有效授权；服务错误时保留原始诊断。本包没有持久数据，回退旧源码无需处理数据迁移。
