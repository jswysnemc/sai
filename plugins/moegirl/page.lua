local http = require("http")
local M = {}
local MAX_OUTPUT_CHARS = 20000

--- 【百科查询】【正文截断】按 Unicode 字符截断并保留原版提示，不截断 UTF-8 字节序列
--- @param value string Markdown 正文
--- @return string 最多指定字符数及必要的截断提示
function M.clip(value)
    local after = utf8.offset(value, MAX_OUTPUT_CHARS + 1)
    if after and after <= #value then
        return value:sub(1, after - 1) .. "\n...[truncated to 20000 chars]"
    end
    return value
end

--- 【百科查询】【解析接口回退】从 MediaWiki 解析结果读取 HTML，空内容明确失败
--- @param site table 站点地址
--- @param title string 页面名称
--- @return string HTML 正文
local function via_api(site, title)
    local data = http.json(site.api .. "/api.php?action=parse&page=" .. sai.text.url_encode(title)
        .. "&prop=text&format=json")
    local parsed = type(data) == "table" and data.parse or nil
    local text = type(parsed) == "table" and parsed.text or nil
    local html = type(text) == "table" and text["*"] or nil
    assert(type(html) == "string" and sai.text.trim(html) ~= "", "Moegirlpedia page not found or returned empty content")
    return html
end

--- 【百科查询】【页面读取】REST 传输或正文大小失败时回退 API，再生成带来源的 Markdown
--- @param site table 站点地址
--- @param title string 页面名称
--- @return string 来源地址和截断后的 Markdown；HTTP 状态错误直接失败
function M.fetch(site, title)
    assert(sai.text.trim(title) ~= "", "query or title is required")
    local ok, response = pcall(sai.http.request,
        http.options(site.base .. "/rest.php/v1/page/" .. sai.text.url_encode(title) .. "/html", 524288))
    local html = ok and http.body(response) or via_api(site, title)
    return "Source: " .. site.page .. sai.text.url_encode(title) .. "\n\n" .. M.clip(sai.text.html_to_markdown(html))
end

return M
