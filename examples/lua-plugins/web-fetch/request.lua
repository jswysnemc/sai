local M = {}
local ACCEPT = {
    text = "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1",
    html = "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, */*;q=0.1",
    markdown = "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1",
}

--- 【网页读取】【网络请求】读取最多五 MiB 原始字节，按原请求头访问已知地址
--- @param options table 经过归一的地址、格式及毫秒期限
--- @return table 成功响应的状态、头部和二进制正文；失败时抛出错误
function M.fetch(options)
    -- 1. 【网页读取】【零期限】零秒立即失败，不能被普通网络接口抬高为正数期限
    if options.timeout_ms == 0 then
        error("web fetch request timed out")
    end
    local ok, response = pcall(sai.binary.request, {
        url = options.url,
        method = "GET",
        timeout_ms = options.timeout_ms,
        max_bytes = 5 * 1024 * 1024,
        max_redirects = 10,
        read_error_body = false,
        headers = {
            ["User-Agent"] = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36",
            ["Accept"] = ACCEPT[options.format] or ACCEPT.markdown,
            ["Accept-Language"] = "en-US,en;q=0.9",
        },
    })
    -- 2. 【网页读取】【响应边界】保留五 MiB 提示，其他宿主错误沿用已经脱敏的内容
    if not ok then
        local message = tostring(response)
        if message:find("HTTP response exceeds byte limit", 1, true)
            or message:find("binary response exceeds size limit", 1, true) then
            error("response too large (exceeds 5MB limit)")
        end
        error(message)
    end
    -- 3. 【网页读取】【状态处理】错误状态不读取正文，也不把查询参数带入错误消息
    if response.status >= 400 and response.status < 600 then
        response.body:close()
        local category = response.status < 500 and "client" or "server"
        error("HTTP status " .. category .. " error (" .. response.status .. ")")
    end
    return response
end

return M
