local http = require("http")
local candidate = require("candidate")
local M = {}

--- 【网页搜图】【查询令牌】按原标记优先级提取数字与连字符组成的 vqd
--- @param html string 搜索入口正文
--- @return string|nil 查询令牌
function M.vqd(html)
    for _, marker in ipairs({'vqd="', "vqd='", 'vqd:"', "vqd: '", '"vqd":"'}) do
        local start = html:find(marker, 1, true)
        if start then
            local value = html:sub(start + #marker):match("^[0-9%-]+")
            if value then return value end
        end
    end
    return nil
end

--- 【网页搜图】【DuckDuckGo 解析】只检查原数量范围内的结果，拒绝非 HTTP(S) 图片
--- @param body string 引擎 JSON 正文
--- @param limit integer 候选池上限
--- @return table 候选数组
function M.parse(body, limit)
    local data = sai.json.decode(body)
    local results = type(data) == "table" and data.results or nil
    local candidates = sai.json.array()
    if type(results) ~= "table" then return candidates end
    for index = 1, math.min(#results, limit) do
        local item = results[index]
        if type(item) == "table" then
            local value = candidate.build(item.title, item.url, item.image, item.thumbnail,
                "DuckDuckGo Images", item.width, item.height, "")
            if value then candidates[#candidates + 1] = value end
        end
    end
    return candidates
end

--- 【网页搜图】【DuckDuckGo 请求】先获取令牌，再请求图片候选
--- @param config table 包设置
--- @param query string 用户查询
--- @param limit integer 候选池上限
--- @param safe boolean 是否启用安全搜索
--- @return table 候选数组
function M.search(config, query, limit, safe)
    local base = config.duckduckgo_base_url
    local page = base .. "/?" .. http.query({{"q", query}, {"iax", "images"}, {"ia", "images"}})
    local token = assert(M.vqd(http.get(config, page)), "DuckDuckGo image page did not return vqd")
    local url = base .. "/i.js?" .. http.query({{"q", query}, {"o", "json"}, {"p", safe and "1" or "-1"},
        {"s", "0"}, {"u", "bing"}, {"f", ",,"}, {"l", "us-en"}, {"vqd", token}})
    local referer = base .. "/?q=" .. sai.text.url_encode(query) .. "&iax=images&ia=images"
    return M.parse(http.get(config, url, referer), limit)
end

return M
