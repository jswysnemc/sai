local common = require("providers.common")
local format = require("format")

--- 【网页搜索】【TinyFish 查询】保留 API Key 请求头及地域、语言查询参数
--- @param input table 归一化后的查询参数
--- @param config table 搜索设置
--- @return string 最多指定数量的 Markdown 搜索结果
local function search(input, config)
    local key = common.key(config, "tinyfish")
    local params = { { "query", input.query } }
    if input.location ~= "" then params[#params + 1] = { "location", input.location } end
    if input.language ~= "" then params[#params + 1] = { "language", input.language } end
    local url = common.append_query(sai.text.trim(config.tinyfish_base_url), params)
    local data = sai.json.decode(common.request(url, "GET", { ["x-api-key"] = key }, nil, config))
    local results = common.results(data, "results", input.max_results)
    assert(#results > 0, "TinyFish returned no results")
    return format.render(input.query, "TinyFish", results)
end

return search
