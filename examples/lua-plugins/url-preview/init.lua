--- 【地址预览】【设置校验】在安装和配置阶段拒绝未定义的设置
---@return nil 本模块注册阶段不发起网络请求
local function validate_settings()
    assert(next(sai.config) == nil, "url-preview does not accept settings")
end
validate_settings()

--- 【地址预览】【正文预览】组合任意来源只读请求和有界 HTML 转换
---@param url string 用户给定的 HTTP(S) 地址
---@param ctx SaiContext 宿主提供的进度与任务上下文
---@return table HTTP 状态、最终地址和最多 2000 字符的 Markdown 或原文摘录
local function preview(url, ctx)
    ctx.progress("Reading URL preview")
    -- 1. 【地址预览】【响应控制】只读取有界正文，错误状态保留状态码并跳过正文
    local response = sai.binary.request({
        url = url, method = "GET", max_bytes = 262144, timeout_ms = 5000,
        max_redirects = 3, read_error_body = false,
        headers = { Accept = "text/html, text/plain;q=0.9" },
    })
    if response.status >= 400 then
        response.body:close()
        return { status = response.status, url = response.url, document = sai.json.null }
    end
    -- 2. 【地址预览】【格式选择】正文类型由插件解释，转换接口不进行 MIME 推断
    local content_type = sai.text.lower(response.headers["content-type"] or "")
    local mode = content_type:find("text/html", 1, true) and "html_markdown" or "raw"
    local ok, document = pcall(response.body.document, response.body, {
        mode = mode, max_chars = 2000, timeout_ms = 3000,
    })
    response.body:close()
    assert(ok, document)
    return { status = response.status, url = response.url, document = document }
end

--- 【地址预览】【工具适配】读取经过工具 Schema 校验的地址
---@param args table 包含 url 字段
---@param ctx SaiToolContext 当前工具上下文
---@return table 预览结果
local function tool(args, ctx)
    return preview(args.url, ctx)
end

--- 【地址预览】【命令适配】把完整命令参数当作地址，不执行 shell 解析
---@param arguments string 用户输入的地址文本
---@param ctx SaiContext 当前命令上下文
---@return table 预览结果
local function command(arguments, ctx)
    local url = sai.text.trim(arguments)
    assert(url ~= "", "usage: plugins run url-preview read <url>")
    return preview(url, ctx)
end

sai.register_tool({
    name = "preview", description = "Preview a user-selected HTTP(S) URL as bounded text.",
    access = "read_only",
    parameters = {
        type = "object", properties = { url = { type = "string", minLength = 1 } },
        required = { "url" }, additionalProperties = false,
    },
    execute = tool,
})
sai.register_command({
    name = "read", description = "Preview the URL supplied as command arguments.",
    access = "read_only", execute = command,
})
