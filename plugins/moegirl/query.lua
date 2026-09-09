local sites = require("sites")
local http = require("http")
local page = require("page")
local M = {}

--- 【百科查询】【查询编排】按 search、page 或自动模式完成搜索与页面读取
--- @param args table 可选 query、title、mode 和 site
--- @return string 原始搜索 JSON 或带来源的 Markdown 页面
function M.run(args)
    local mode, site = args.mode or "auto", sites.select(args.site or "zh")
    local query, title = sai.text.trim(args.query or ""), sai.text.trim(args.title or "")
    if mode == "search" or (mode == "auto" and title == "") then
        local term = query == "" and title or query
        assert(term ~= "", "query or title is required")
        local data = http.json(site.api .. "/api.php?action=opensearch&search=" .. sai.text.url_encode(term)
            .. "&limit=5&namespace=0&format=json")
        if mode == "search" then return sai.json.encode(data) end
        local titles = type(data) == "table" and data[2] or nil
        local first = type(titles) == "table" and titles[1] or nil
        if type(first) == "string" then return page.fetch(site, first) end
    end
    return page.fetch(site, title == "" and query or title)
end

return M
