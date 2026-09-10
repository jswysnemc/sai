# 二进制与终端图片接口

图片正文使用独立缓冲，不经过 Lua JSON 工具结果通道。生成请求、搜索与排序、下载编排、图片元数据、视觉筛选规则和预览策略属于插件；Rust 提供受控网络、字节缓冲、文件输出、单次视觉请求及终端绘制。

## 能力与授权

```json
{
  "capabilities": {
    "http": ["https://api.example.com"],
    "binary": {
      "public_downloads": true,
      "write_paths": ["output/images"],
      "display_images": true
    }
  },
  "limits": {
    "binary_bytes": 33554432,
    "binary_timeout_ms": 180000,
    "timeout_ms": 400000
  }
}
```

三个二进制能力分别声明、分别授权。外部插件默认没有这些授权，启用本身不会授予能力：

```sh
sai plugins enable example --allow-public-downloads
sai plugins enable example --allow-write-path output/images
sai plugins enable example --allow-image-display
sai plugins enable example --no-public-downloads
sai plugins enable example --no-file-write
sai plugins enable example --no-image-display
```

重复的 `--allow-write-path` 替换目录集合，未指定的能力保持原值。`--grant-declared` 与所有分项选项互斥，相应的授权和撤销选项也互斥。修改输出目录不会把旧显式授权自动转移到新目录。声明和授权按字符串求交集，实际文件归属由宿主进一步验证。

`--no-http` 只撤销精确 HTTP 来源和只读 POST 端点；要同时关闭匿名公开下载，另加 `--no-public-downloads`。公开下载能力不能用于任意方法或带凭据请求，也不会开放 `sai.http.request`。

## 请求和缓冲

```lua
local response = sai.binary.request({
    url = "https://api.example.com/images",
    method = "POST",
    headers = {["content-type"]="application/json"},
    body = sai.json.encode({prompt="example"}),
    max_bytes = 16 * 1024 * 1024,
    timeout_ms = 180000,
})
assert(response.status == 200, response.body:text(1024))
local image = assert(response.body:json_base64("/data/0/b64_json"))
response.body:close()
local saved = image:write("output/images/example.png")
image:close()
return saved
```

`sai.binary.request` 的请求字段与文本 HTTP 相同，使用相同精确来源、只读 POST、重定向和凭据检查。两个二进制网络入口返回 `{status, url, headers, body}`，`url` 是最终重定向地址，`body` 是不可序列化的 Lua userdata。状态错误仍作为响应交给插件。

`sai.binary.download({url, max_bytes?, timeout_ms?})` 只接受无正文、无自定义请求头的 GET。每次跳转都重新检查地址；精确授权的来源可以是本地服务，其他来源必须具有 `public_downloads` 授权，且全部 DNS 结果均为公开单播地址。宿主固定实际连接地址并禁用公开下载的代理，防止解析检查与连接目标不一致。每条链最多跟随五次跳转，共用截止时间；不会保存或转发 Cookie。

两个入口默认请求大小为 1 MiB、默认请求时限为 30 秒；实际大小收窄到 VM 剩余二进制预算，时限收窄到 `binary_timeout_ms`。请求正文与元数据仍受文本 `output_bytes` 限制。响应原始字节不受文本输出上限限制，也不按 charset 解码。

| 接口 | 返回值与边界 |
| --- | --- |
| `buffer:len()` | 原始字节数 |
| `buffer:bytes(offset, length)` | 从零基偏移读取原始 Lua 字符串，保留零字节和无效 UTF-8；只接受非负整数，长度不能超过 `output_bytes`，超出缓冲尾部返回已有部分 |
| `buffer:sha256()` | 小写十六进制 SHA-256；消耗一次系统调用额度，在阻塞线程计算，完成前继续持有字节预算 |
| `buffer:text(max_bytes?)` | 有界 UTF-8 文本前缀；无效编码替换，不超过 `output_bytes` |
| `buffer:json_type(pointer)` | `object`、`array`、`string`、`number`、`boolean`、`null`；字段缺失返回 nil |
| `buffer:json_string(pointer)` | 字符串或 nil；文本超过输出限制时失败 |
| `buffer:json_base64(pointer)` | 将字符串字段解码为新缓冲，字段缺失或不是字符串时返回 nil；非法 Base64 失败 |
| `buffer:write(path)` | `{path, bytes}`；必须同时取得目录授权和本次回调写入权限 |
| `buffer:close()` | 立即释放句柄引用；重复关闭没有副作用 |
| `sai.binary.decode_base64(text)` | 从有界 Lua 文本创建缓冲；输入受 `output_bytes` 限制 |

JSON 路径遵循 RFC 6901，最多 1024 字节、64 层。解析只选取目标字段，不把完整响应复制成 JSON 对象树；重复键采用最后一个值。所有原始缓冲和解码副本共享同一预算，包含仍被宿主 I/O 持有的缓冲。转义字符串需要额外解码空间，空间不足时拒绝操作。

句柄只在创建它的回调内有效。回调完成、失败或取消都会释放句柄中的数据；Lua 保存旧句柄也不能在下一次回调重新使用。宿主文件线程持有的独立租约在实际 I/O 结束后才归还预算。显式关闭仍有助于在同一回调内及时释放空间。

## 文件输出和取消

目录声明最多 64 项，支持绝对路径、可信工作目录下的相对路径及 `~/`。不接受父目录跳转、通配符、控制字符；输出路径另拒绝 Windows 设备名称、数据流名称和尾部规范化歧义。

宿主先解析现有祖先并验证完整目标归属，再创建必要目录。后续逐层使用不跟随链接的目录句柄，禁止通过越界链接写入文件。数据写入同目录随机暂存文件，完整写入并成功等待后才原子替换目标。

取消会撤销异步调用并通知文件线程。已开始的系统调用不能保证立即中断，线程在数据块边界检查取消，保留缓冲预算直至退出；取消或错误不发布暂存文件。授权内新建的空目录可能保留。

## 视觉模型

视觉能力独立于文本模型、网络和终端授权。包须声明 `"vision": true`，外部包再通过 `--allow-vision` 授权；`--no-vision` 仅撤销视觉能力。普通 `--allow-model` 不会间接开放视觉请求。

```lua
local info = sai.vision.info()
local response = image:analyze_image({
    system = "只描述图片中可见的内容",
    prompt = "这张图片展示了什么？",
    mime_type = "image/png",
    timeout_ms = 10000,
})
return {content=response.content, provider_id=response.provider_id, model=response.model}
```

`info()` 返回 `{provider_id, model}`，宿主关闭视觉时返回 JSON null；配置无效时返回错误。它不初始化模型客户端，也不读取密钥文件。视觉供应商由旧 `plugins.vision.vision_provider_id` 选择，空值使用主配置中的活动供应商；`vision_model` 非空时覆盖该供应商默认模型。文本模型切换和工具表过滤保留这项独立配置。Lua 不获得地址、凭据或主配置，也不能通过请求覆盖它们。

`analyze_image` 只接受 `system`（默认空字符串）、非空 `prompt`、`mime_type` 和可选 `timeout_ms`。支持 PNG、JPEG/JPG、GIF、WebP、BMP；真实缓冲必须为 1 字节至 10 MiB。未知字段、路径、供应商或模型覆盖均被拒绝。图片由当前有效缓冲提供，无任意文件读取入口；是否匹配用户查询、如何处理模型输出以及是否保留失败图片由 Lua 决定。

视觉与文本共用每次回调的 `model_requests` 次数；输入、响应、流式正文、思考和工具参数受 `output_bytes` 限制。宿主发送单次无工具图片请求，不自动执行模型建议。`timeout_ms` 默认使用回调总时长，限制在 1 毫秒至该上限。超时可由 `pcall` 捕获，超时和外部取消均释放模型 Future；图片租约覆盖整个请求。初始化、事件和通知纯回调不能使用视觉服务，完成或取消后的旧句柄也不能复用。

## 终端图片

```lua
local terminal = sai.terminal.size()
local shown = sai.terminal.display_image("output/images/example.png", "80x40")
return shown.path
```

尺寸查询与绘制都要求 `display_images` 授权并处于有效回调。`size()` 返回 `{columns, rows}`，没有交互终端时为 nil。绘制返回 `{path}`，不向 Lua 提供本地文件内容；这项能力授权宿主读取并展示本地图片，不授予通用文件读取能力。

宿主使用不超过 64 MiB 的普通文件快照，验证可识别图片及像素上限：每边最多 16384 像素，总计最多 32 Mi 个像素。单元格尺寸最多 300 列、200 行；支持 `WIDTHxHEIGHT`、`WIDTHx`、`xHEIGHT`。渲染继续使用已有终端协议与降级器，Kitty 与 iTerm 均按传入尺寸限制显示范围。图片数据与放置指令完整生成后才一起输出，丢弃渲染结果不会消耗传输缓存。取消后不提交迟到输出，已经开始的阻塞读取或渲染需要自然结束。

## 内置图片包

`image-generation` 保留 `generate_image`、原 OpenAI/RightCode 请求规则、文件名称及全部结果字段。旧 `plugins.image_generation` 仅为此包提供逐字段默认值，显式设置优先，包括 `auto_print=false`。管理操作只保存显式业务设置，不复制旧凭据或固化旧默认值；明确授予声明能力时，会保存当前 API 来源与输出目录授权。

默认生成请求超时仍为 180 秒，允许设置 1–600 秒；API 响应和图片下载各最多 32 MiB，VM 二进制总预算 64 MiB，工具输出 64 KiB，完整回调最多 1300 秒。长请求使用独立二进制时限，普通文本 HTTP 的 120 秒硬上限保持原规则。

`image-display` 保留只读 `print_image`。宽高参数优先于 `size`，再回退到终端百分比；默认 45% 宽、35% 高，沿用旧 `plugins.print_image` 设置。显式插件设置可覆盖 `width_percent`、`height_percent` 和 `language`（`en` 或 `zh`）。

自动预览通过 `sai.tools.list/call` 调用 `print_image`，因此显示包的开关、授权、尺寸和 Agent 工具白名单都参与生效。显示工具不可用时跳过预览，绘制失败时返回 `printed=false` 和 `print_error`，已经保存的图片仍然成功。`generate_image` 保持写入属性，不进入只读工具目录；`print_image` 可以在只读目录中使用。

`web-images` 保留 `search_web_images`，声明 `optional_writes`。只读模式只返回远程候选元数据；普通模式按用户数量搜索、稳定排序、去重、下载，再根据独立视觉结果筛选。DuckDuckGo 失败或数量不足时回退 Bing；原图失败时尝试缩略图，内容摘要决定 `webimg-<sha256>.<ext>` 名称。审核拒绝后继续候选，审核失败保留图片和失败记录。

旧 `plugins.web_images` 提供逐字段默认值。内置缓存默认位于应用图片目录的 `web-images` 子目录，允许显式 `cache_dir`；`duckduckgo_base_url` 与 `bing_base_url` 可配置为独立实例。兼容层只派生精确搜索来源和输出目录声明，已有显式授权不会跟随设置改变；外部包不继承旧设置或目录。

包最多返回 10 张图片、尝试 16 个下载候选、预览 5 张。单次下载由 `max_download_mb` 限制到 0.1–64 MiB；搜索正文最多 4 MiB，VM 二进制预算为 64 MiB，回调上限 3600 秒，视觉最多 16 次。预览继续通过独立显示包，使用 `printed` 和 `print_errors` 报告结果；显示失败不丢失已保存文件。
