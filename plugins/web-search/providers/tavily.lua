local common = require("providers.common")
local format = require("format")

--- 【网页搜索】【Tavily 查询】传递搜索深度、答案与原始正文选项
--- @param input table 归一化后的查询参数
--- @param config table 搜索设置
--- @return string 与原版空结果语义一致的 Markdown
local function search(input, config)
    local data = common.post("tavily", {
        query = input.query, max_results = math.min(input.max_results, 20),
        search_depth = config.tavily_search_depth,
        include_answer = config.tavily_include_answer,
        include_raw_content = config.tavily_include_raw_content and "markdown" or false,
    }, config)
    return format.render(input.query, "Tavily", common.results(data, "results"))
end

return search
