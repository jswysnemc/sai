---@meta

---@alias SaiDocumentMode 'raw'|'html_text'|'html_markdown'

---@class SaiDocumentOptions
---@field mode? SaiDocumentMode 默认 raw，HTML 格式由插件根据业务选择
---@field max_chars? integer 正整数，默认 24000，按 Unicode 标量计数
---@field width? integer 20–200，默认 120，仅控制 HTML 纯文本换行
---@field timeout_ms? integer 正整数毫秒，默认并收窄到 binary_timeout_ms，包含排队时间

---@class SaiDocument
---@field text string UTF-8 替换解码后的有限摘录，不附加截断提示
---@field total_chars integer 完整转换结果的 Unicode 标量数量
---@field truncated boolean 摘录是否少于完整结果

--- 当前回调的不透明缓冲句柄，不能直接序列化或跨回调复用
---@class SaiBuffer
---@field len fun(self: SaiBuffer): integer 返回完整原始字节数
---@field text fun(self: SaiBuffer, max_bytes?: integer): string 返回有界 UTF-8 替换解码前缀
---@field document fun(self: SaiBuffer, options?: SaiDocumentOptions): SaiDocument 转换至多 8 MiB 输入，完整 JSON 结果仍受 output_bytes 限制
---@field close fun(self: SaiBuffer): nil 释放句柄引用；仍在运行的原生工作继续持有资源

---@class SaiBinaryResponse
---@field status integer 最终 HTTP 状态码
---@field headers table<string, string> 正式宿主使用小写响应头名称
---@field url string 最终重定向地址
---@field body SaiBuffer 当前回调的原始正文缓冲

---@class SaiBinary
---@field request fun(request: SaiHttpRequest): SaiBinaryResponse 使用来源授权请求原始字节，不按 HTTP charset 解码
---@field from_bytes fun(bytes: string): SaiBuffer 从有界 Lua 原始字符串构造缓冲，保留无效 UTF-8 和零字节
