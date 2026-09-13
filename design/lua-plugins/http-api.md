# HTTP 公共契约

`sai.http.request` 读取解码后的文本，`sai.binary.request` 保留原始字节。两者共用请求参数、来源授权、重定向和凭据规则。完整外部包见 [URL 预览示例](../../examples/lua-plugins/url-preview/README.md)。

## 场景与授权

已知服务使用精确来源 `http`。如果工具接受用户任意指定的 URL，而地址不能在开发时列举，可以独立声明 `http_read_any`：

```json
{"capabilities":{"http_read_any":true}}
```

```sh
sai plugins enable url-preview --allow-http-read-any
sai plugins enable url-preview --no-http-read-any
sai plugins enable url-preview --no-http
```

`http_read_any` 只扩大 GET/HEAD 的来源范围，包含回环、局域网和公网 HTTP(S) 服务，不等同于“仅公网”。它必须同时存在于清单和用户授权中；写入回调也不能借此向任意来源发送 POST、PUT、PATCH 或 DELETE。需要认证时可以在初始请求提供请求头，URL 内嵌用户名或密码仍然拒绝。

`--no-http-read-any` 只撤销任意来源读取，保留精确来源和查询 POST 授权。`--no-http` 撤销精确来源、查询 POST 和任意来源读取，保留其他能力。普通 `enable` 不恢复已撤销权限。

| 能力 | 方法及来源 | 只读调用 |
| --- | --- | --- |
| `http` | 精确规范来源，如 `https://api.example.com`，不含路径或尾部 `/` | GET/HEAD；其他方法还需可信写入许可或下项查询授权 |
| `http_read_only_post` | 精确规范端点，且所属来源也在有效 `http` 中 | 只开放该端点的 POST；不覆盖子路径，查询参数不参与端点匹配 |
| `http_read_any` | 任意合法 HTTP(S) 来源，包括本地服务 | 只开放 GET/HEAD |
| `binary.public_downloads` | `sai.binary.download` 的匿名公开下载 | 不提供自定义头或正文；未精确授权的来源必须通过公网 DNS/地址检查 |

任意来源读取不会自动授权匿名下载；匿名下载继续使用精确来源或 `binary.public_downloads`，不继承 `http_read_any`。精确来源和只读 POST 的声明、分项授权见[开发指南](getting-started.md#配置与授权)。

私有工作目录的 `work:extract_tar_gz` 下载阶段也复用 GET 来源授权，可以使用 `http_read_any`；创建工作目录仍需独立的 `system.workspace`。归档保持固定五次跳转与自己的大小、条目和展开限制，不接收本页新增请求选项，详见[归档契约](private-api.md#归档展开)。

## 参数与返回值

```lua
local response = sai.http.request({
    url = "https://api.example.com/status",
    method = "GET",
    headers = { Accept = "application/json" },
    max_bytes = 65536,
    timeout_ms = 5000,
    max_redirects = 3,
    read_error_body = false,
})
```

| 字段 | 类型、缺省值和边界 |
| --- | --- |
| `url` | 必填 UTF-8 字符串；HTTP(S)、有效主机、无内嵌凭据；最多 8192 字节 |
| `method` | 字符串，缺省 GET；不区分输入大小写，支持 GET、HEAD、POST、PUT、PATCH、DELETE |
| `headers` | 字符串到字符串的对象，缺省空；最多 32 项、合计 16 KiB；传输头由宿主管理 |
| `body` | 可选字符串；受 `limits.output_bytes` 限制；授权按方法与回调写入许可判断 |
| `max_bytes` | 非负整数，缺省 1 MiB；实际收窄到至少 1 字节及文本输出上限或剩余二进制额度；超出有效上限是错误，不截断响应 |
| `timeout_ms` | 非负整数毫秒，缺省 30000；实际收窄到至少 1 毫秒及 `http_timeout_ms` 或 `binary_timeout_ms`；不能延长回调期限 |
| `max_redirects` | 整数 0–10，缺省 5；超限、负数、分数和数字字符串均拒绝 |
| `read_error_body` | 布尔值，缺省 true；false 只跳过最终 4xx/5xx 的正文 |

请求拒绝未知字段。省略 `max_redirects` 和 `read_error_body` 保留既有的五次跳转与错误正文读取行为。二进制下载也接受这两个选项，但仍只允许无正文、无自定义头的 GET。

文本返回 `{status: integer, headers: table<string,string>, text: string}`；正式宿主的响应头键采用小写。文本按照 Content-Type charset 解码，解码前后分别检查大小。二进制返回 `{status, headers, url, body}`，`url` 是最终地址，`body` 是当前回调的缓冲句柄，不能直接序列化；详见[二进制接口](binary-api.md)。

HTTP 4xx/5xx 本身不是 Lua 异常。`read_error_body=false` 时返回原状态、响应头及空文本/空缓冲，不等待错误正文，也不因其 Content-Length 很大而失败。成功状态仍执行完整正文大小检查。调用者据状态决定业务错误或重试；该选项不隐藏网络、超时或权限错误。

## 重定向与取消

宿主跟随 301、302、303、307、308 中有效的 Location。达到 `max_redirects` 后再收到可跟随的重定向会返回错误，不连接下一跳；`0` 表示遇到这种重定向就失败，不用于返回 3xx 元数据。没有 Location 的响应原样交付。

每一跳重新检查方法、地址及来源权限。POST 遇到 301/302，或非 HEAD 遇到 303 时转为 GET 并移除正文和相关内容头。保留 POST 的跳转仍须有目标端点授权。跨来源不转发正文，仅保留 Accept、Accept-Language、User-Agent；认证及自定义敏感头在第一跳之后删除。

整条链共用请求截止时间，正文读取也在剩余期限内。回调取消释放等待中的受管网络 Future；单次请求超时可以用 `pcall` 捕获。二进制请求进入宿主后会消耗一次系统调用额度，失败不退还；文本请求不使用这个次数额度，但仍受字节和时长限制。HTTP 写入可能已在远端完成，取消不表示远端回滚。

## 验证与兼容影响

运行时契约覆盖声明/授权交集、方法和 URL 拒绝、选项类型、默认值、网络取消及资源限制。正式宿主测试覆盖普通外部身份、跨来源凭据删除、跳转上限和状态先于正文读取；CLI 示例使用本地服务验证显式授权及撤销。

本次没有改变既有精确来源的缺省范围。新增任意来源权限默认 false，升级清单不会自动取得授权。旧宿主不认识新清单或请求字段时会报错；按[兼容规则](compatibility.md)记录最低能力要求，不用插件自身 SemVer 推断宿主支持。
