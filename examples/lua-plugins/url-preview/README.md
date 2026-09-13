# URL 预览插件

本包通过公共 `sai.binary.request` 与 `buffer:document` 读取用户指定地址，返回至多 2000 个字符的 Markdown 或原文。需要包含 `http_read_any` 和正文转换接口的 v1 构建；旧版在检查新清单时会报告未知能力。

复制或解压本目录后执行：

```sh
sai plugins check ./url-preview
sai plugins install ./url-preview
sai plugins enable url-preview --allow-http-read-any
sai --plan plugins run url-preview read https://example.com/
```

工具名为 `lua__url-preview__preview`，参数为 `{"url":"https://example.com/"}`。不需要模型、私有兼容身份或修改 Rust。命令把完整参数作为 URL，包含 `&` 等 shell 特殊字符的地址应按所用 shell 的规则引用。

`http_read_any` 允许 GET/HEAD 访问任意 HTTP(S) 来源，包含本地服务；不是仅公网权限。示例只发送 GET，不要求文件、工具、模型、存储或进程授权。初始请求最多读取 256 KiB，允许三次跳转、五秒网络期限；正文转换最多三秒，仍受回调十五秒总期限限制。

正常返回 `{status, url, document}`，`document` 为 `{text,total_chars,truncated}`。4xx/5xx 返回原状态且 `document=null`，不读取可能很大或停滞的错误正文。HTML 根据响应类型由插件选择转为 Markdown；其他正文按 UTF-8 替换解码。重定向过多、正文超限和网络超时会明确失败。

```sh
sai plugins enable url-preview --no-http-read-any
sai plugins disable url-preview
sai plugins remove url-preview
```

撤权后普通启用不会恢复访问。固定服务插件可以将清单改为精确 `http` 来源，然后使用 `--allow-http`；不要为已知单一服务默认请求任意来源权限。本包不保存持久数据。

分发所需文件为 `sai-plugin.json` 与 `init.lua`。修改源码后更新插件版本，使用 `sai plugins pack ./url-preview` 生成源码压缩包，另附本说明和实际验证的 sai 构建。接收者先解压，再 `check` 和 `install`；更新使用 `install --replace`，TUI 会话通过 `/plugins reload` 生效。具体步骤见[分发说明](../../../design/lua-plugins/distribution.md)，授权及错误语义见[HTTP 契约](../../../design/lua-plugins/http-api.md)，转换限制见[正文契约](../../../design/lua-plugins/document-api.md)。

本包的参数与返回值可使用[LuaLS 类型提示](../../../design/lua-plugins/editor-support.md)，类型声明不属于插件运行依赖。

仓库内的真实 Linux CLI 验收使用本地 HTTP 样本：

```sh
uv run --no-project python scripts/plugin-smoke/verify_http.py \
  --binary /absolute/path/to/sai \
  --report target/public-http-validation/cli-report.json
```

脚本将包复制到仓库外，用独立 XDG 配置验证未授权不连接、Markdown、Unicode 截断、重定向、错误正文、大小、超时及分项撤权；退出后清理服务和临时目录。
