local common = require("providers.common")
local format = require("format")

--- 【网页搜索】【Firecrawl 查询】请求网页来源与 Markdown 正文，读取原 data 数组
--- @param input table 归一化后的查询参数
--- @param config table 搜索设置
--- @return string 包含元数据地址和正文的 Markdown
local function search(input, config)
    local data = common.post("firecrawl", {
        query = input.query, limit = math.min(input.max_results, 20),
        sources = sai.json.array({ { type = "web" } }),
        scrapeOptions = {
            formats = sai.json.array({ { type = "markdown" } }),
            onlyMainContent = config.firecrawl_only_main_content,
        },
    }, config)
    return format.render(input.query, "Firecrawl", common.results(data, "data"))
end

return search
