local http = require("http")
local M = {}
local base = "https://wiki.archlinux.org/api.php"

--- 【ArchWiki】【页面读取】获取解析后的正文，保留 Markdown 链接
--- @param title string 页面标题
--- @return string Markdown 正文
local function page(title)
    assert(sai.text.trim(title) ~= "", "query or title is required")
    local data = http.json(base .. "?action=parse&page=" .. sai.text.url_encode(title)
        .. "&prop=text&format=json")
    local parsed = type(data.parse) == "table" and data.parse or {}
    local text = type(parsed.text) == "table" and parsed.text or {}
    local html = type(text["*"]) == "string" and text["*"] or ""
    return sai.text.html_to_markdown(html)
end

--- 【ArchWiki】【查询】搜索标题或读取指定页面，自动模式读取首个匹配项
--- @param args table 含 mode、title 和 query
--- @return table|string 原搜索结果或 Markdown 正文
function M.query(args)
    local mode = args.mode or "auto"
    local title = sai.text.trim(args.title or "")
    local query = sai.text.trim(args.query or "")
    if mode == "search" or (mode == "auto" and title == "") then
        local term = query ~= "" and query or title
        local data = http.json(base .. "?action=opensearch&search=" .. sai.text.url_encode(term)
            .. "&limit=8&namespace=0&format=json")
        if mode == "search" then return data end
        local titles = type(data[2]) == "table" and data[2] or {}
        if type(titles[1]) == "string" then return page(titles[1]) end
    end
    return page(title ~= "" and title or query)
end

return M
