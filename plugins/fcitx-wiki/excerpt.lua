local M = {}
local max_chars = 12000

--- 【Fcitx Wiki】【正文范围】定位正文容器并匹配完整嵌套 div
--- @param html string 官方页面 HTML
--- @return string 正文 HTML；缺少有效容器时使用原页面
local function body(html)
    local position = 1
    while true do
        local start, finish = html:find("<[dD][iI][vV]%f[%s/>][^>]*>", position)
        if not start then return html end
        local tag = html:sub(start, finish)
        local _, class = tag:match("[cC][lL][aA][sS][sS]%s*=%s*([\"'])(.-)%1")
        if class and (" " .. class .. " "):find("%s+mw%-parser%-output%s+") then
            local content_start = finish + 1
            local depth = 1
            position = content_start
            while true do
                local next_start, next_end, closing = html:find(
                    "<%s*(/?)%s*[dD][iI][vV]%f[%s/>][^>]*>", position)
                if not next_start then return html end
                if closing == "/" then
                    depth = depth - 1
                    if depth == 0 then return html:sub(content_start, next_start - 1) end
                elseif not html:sub(next_start, next_end):match("/%s*>$") then
                    depth = depth + 1
                end
                position = next_end + 1
            end
        end
        position = finish + 1
    end
end

--- 【Fcitx Wiki】【页面摘录】读取有大小限制的页面，保留 Markdown 并按 Unicode 截断
--- @param url string 白名单页面地址
--- @return string 最多 12000 个正文字符及必要的截断说明
function M.fetch(url)
    local response = sai.http.request({
        url = url,
        method = "GET",
        headers = { ["user-agent"] = "sai/0.1 fcitx5_input_method_wiki_qurey" },
        max_bytes = 512 * 1024,
        timeout_ms = 12000,
    })
    assert(response.status >= 200 and response.status < 300,
        "Fcitx Wiki returned HTTP " .. response.status)
    local text = sai.text.html_to_markdown(body(response.text))
    local cutoff = utf8.offset(text, max_chars + 1)
    if cutoff and cutoff <= #text then
        return text:sub(1, cutoff - 1) .. "\n...[truncated to " .. max_chars .. " chars]"
    end
    return text
end

return M
