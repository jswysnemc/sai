local http = require("http")
local M = {}
local base = "https://94he6yatei-dsn.algolia.net/1/indexes/steamdb"
local search_key = "9ba0e69fb2974316cdaec8f5f257088f"

--- 【ProtonDB】【游戏搜索】通过 Algolia GET 接口检索第一个游戏命中
--- @param query string 游戏名称或 App ID
--- @return table|nil 首个游戏条目
local function lookup(query)
    local url = base .. "?query=" .. sai.text.url_encode(query)
        .. "&facetFilters=" .. sai.text.url_encode(sai.json.encode({ { "appType:Game" } }))
        .. "&hitsPerPage=1&attributesToRetrieve="
        .. sai.text.url_encode(sai.json.encode({ "name", "objectID", "oslist" })) .. "&page=0"
    local data = http.json(url, {
        ["x-algolia-api-key"] = search_key,
        ["x-algolia-application-id"] = "94HE6YATEI",
        Referer = "https://www.protondb.com",
    })
    local hits = type(data.hits) == "table" and data.hits or {}
    return type(hits[1]) == "table" and hits[1] or nil
end

--- 【ProtonDB】【平台列表】只保留接口返回的字符串平台名
--- @param hit table 搜索命中条目
--- @return table 平台名称数组
local function platforms(hit)
    local result = sai.json.array()
    for _, value in ipairs(type(hit.oslist) == "table" and hit.oslist or {}) do
        if type(value) == "string" then result[#result + 1] = value end
    end
    return result
end

--- 【ProtonDB】【整数标识】将十进制 App ID 转换为不丢失精度的 Lua 整数
--- @param value any 搜索结果或用户提供的标识
--- @return integer|nil 有效整数标识
local function app_id(value)
    if type(value) ~= "string" or not value:match("^%d+$") then return nil end
    local number = tonumber(value)
    return number and math.tointeger(number) or nil
end

--- 【ProtonDB】【游戏解析】数字查询直接使用 App ID，名称查询取第一个游戏结果
--- @param query string 已修剪的用户查询
--- @return integer Steam App ID
--- @return string 游戏名称或原数字查询
--- @return table 平台名称数组
function M.resolve(query)
    if query:match("^%d+$") then
        local id = assert(app_id(query), "invalid Steam App ID")
        local ok, hit = pcall(lookup, query)
        if ok and hit and hit.objectID == query then
            return id, type(hit.name) == "string" and hit.name or query, platforms(hit)
        end
        return id, query, sai.json.array()
    end
    local hit = assert(lookup(query), 'no search results for "' .. query .. '"')
    local id = assert(app_id(hit.objectID), "invalid objectID in search result")
    return id, type(hit.name) == "string" and hit.name or "unknown", platforms(hit)
end

return M
