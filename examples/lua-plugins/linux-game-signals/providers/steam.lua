local common = require("providers.common")
local M = {}

--- 【游戏信号】【Steam 查询】保留首项匹配规则及原始名称、标识和缩略图字段
--- @param game string 查询名称
--- @return table Steam 匹配信息，没有结果时抛出错误
local function search(game)
    local data = common.json("https://store.steampowered.com/api/storesearch/?term="
        .. sai.text.url_encode(game) .. "&l=english&cc=US")
    local items = common.field(data, "items")
    local first
    if type(items) == "table" then first = items[1] end
    assert(first ~= nil, "Steam app not found for " .. game)
    return {
        appid = common.field(first, "id"),
        name = common.field(first, "name"),
        url = common.field(first, "tiny_image"),
    }
end

--- 【游戏信号】【Steam 候选】返回首个成功结果，失败尝试不会阻止其余来源
--- @param candidates table 查询名称数组
--- @return table|nil Steam 信息，无成功结果时返回 nil
--- @return table 查询尝试记录
function M.fetch(candidates)
    local attempts = sai.json.array()
    for _, candidate in ipairs(candidates) do
        local ok, value = pcall(search, candidate)
        if ok then
            attempts[#attempts + 1] = {
                query = candidate, ok = true, appid = value.appid, name = value.name,
            }
            return value, attempts
        end
        attempts[#attempts + 1] = { query = candidate, ok = false, error = tostring(value) }
    end
    return nil, attempts
end

return M
