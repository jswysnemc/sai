local M = {}

--- 【网页读取】【内容输出】只对大小写匹配的 HTML 类型转换，按完整字符数添加原裁剪提示
--- @param response table 含响应头和二进制正文的成功响应
--- @param options table 输出格式与字符上限
--- @return string 原文、转换正文或带裁剪说明的前缀
function M.render(response, options)
    -- 1. 【网页读取】【格式选择】忽略 charset，XHTML 和其他内容类型保持原文
    local mode = "raw"
    local content_type = response.headers["content-type"] or ""
    if content_type:find("text/html", 1, true) and options.format ~= "html" then
        mode = options.format == "text" and "html_text" or "html_markdown"
    end
    local ok, document = pcall(response.body.document, response.body, {
        mode = mode,
        width = 120,
        max_chars = options.max_chars,
        timeout_ms = 30000,
    })
    response.body:close()
    if not ok then
        error(tostring(document))
    end
    -- 2. 【网页读取】【字符裁剪】宿主返回转换后的完整计数，提示由业务模块组合
    if document.truncated then
        return document.text .. "\n\n[content truncated from " .. document.total_chars
            .. " chars to " .. options.max_chars .. " chars]"
    end
    return document.text
end

return M
