---@meta

--- 文本和二进制请求共用参数，宿主拒绝未知字段并复核每一跳权限
---@class SaiHttpRequest
---@field url string HTTP(S) URL，最多 8192 字节，不允许内嵌凭据
---@field method? string 默认 GET；支持 GET、HEAD、POST、PUT、PATCH、DELETE，输入不区分大小写
---@field headers? table<string, string> 最多 32 项、合计 16 KiB，传输头由宿主管理
---@field body? string 可选正文，受 output_bytes 和方法授权限制
---@field max_bytes? integer 默认 1 MiB，按接口字节额度收窄，超限报错而非截断
---@field timeout_ms? integer 默认 30000，收窄到至少 1 毫秒及接口期限
---@field max_redirects? integer 0–10，默认 5；0 在遇到可跟随的跳转时报错
---@field read_error_body? boolean 默认 true；false 跳过最终 4xx/5xx 正文

---@class SaiHttpResponse
---@field status integer 最终 HTTP 状态码，4xx/5xx 不自动抛异常
---@field headers table<string, string> 正式宿主使用小写响应头名称
---@field text string 按 Content-Type charset 解码的有界正文

---@class SaiHttp
---@field request fun(request: SaiHttpRequest): SaiHttpResponse 请求文本，要求有效来源权限；http_read_any 只开放 GET/HEAD
