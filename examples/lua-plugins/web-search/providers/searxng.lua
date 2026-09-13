local common = require("providers.common")
local format = require("format")

--- 【网页搜索】【SearXNG 查询】访问配置实例的 JSON 搜索端点
--- @param input table 归一化后的查询参数
--- @param config table 搜索设置
--- @return string 最多指定数量的 Markdown 搜索结果
local function search(input, config)
    local base = sai.text.trim(config.searxng_base_url):gsub("/+$", "")
    assert(base ~= "", "missing SearXNG base URL")
    local url = base .. "/search?q=" .. sai.text.url_encode(input.query)
        .. "&format=json&language=" .. sai.text.url_encode(sai.text.trim(config.searxng_language))
        .. "&safesearch=" .. config.searxng_safe_search
    local data = sai.json.decode(common.request(url, "GET", { accept = "application/json" }, nil, config))
    local results = common.results(data, "results", input.max_results)
    assert(#results > 0, "SearXNG returned no results")
    return format.render(input.query, "SearXNG", results)
end

return search
