local common = require("providers.common")
local format = require("format")

--- 【网页搜索】【AnySearch 查询】提交查询及数量上限并使用原有通用结果格式
--- @param input table 归一化后的查询参数
--- @param config table 搜索设置
--- @return string Markdown 搜索结果
local function search(input, config)
    local data = common.post("anysearch", {
        query = input.query, max_results = math.min(input.max_results, 20),
    }, config)
    return format.render(input.query, "AnySearch", common.results(data, "results"))
end

return search
