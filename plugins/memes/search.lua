local values = require("values")
local paths = require("paths")
local library = require("library")
local M = {}

--- 【表情检索】【原版计分】按词累加标签和正文分，再匹配未折叠的完整查询
--- @param item table 条目
--- @param query string 原查询
--- @param tags table 已过滤标签
--- @return number 匹配分数
function M.score(item, query, tags)
    query = values.normalize(query .. " " .. table.concat(tags, " "))
    local haystack = values.normalize(table.concat({item.name.zh, item.name.en, item.description,
        item.usage, item.avoid, table.concat(item.tags, " ")}, " "))
    local score = 0.0
    for term in sai.text.collapse_whitespace(query):gmatch("[^ ]+") do
        if haystack:find(term, 1, true) then
            local weight = 1
            for _, tag in ipairs(item.tags) do
                if values.normalize(tag):find(term, 1, true) then weight = 2; break end
            end
            score = score + weight
        end
    end
    if haystack:find(query, 1, true) then score = score + 2 end
    return score
end

--- 【表情检索】【稳定排序】显式使用输入位置打破平分，保留用户覆盖层优先顺序
--- @param config table 设置
--- @param name string 库名
--- @param query string 查询文字
--- @param tags table 标签
--- @param limit integer 结果上限
--- @return table 按分数排序的条目与来源
function M.rank(config, name, query, tags, limit)
    local result = {}
    for position, loaded in ipairs(library.load(config, name)) do
        local score = M.score(loaded.item, query, tags)
        if score > 0 then result[#result + 1] = {score=score, loaded=loaded, position=position} end
    end
    table.sort(result, function(left, right)
        if left.score == right.score then return left.position < right.position end
        return left.score > right.score
    end)
    while #result > math.max(1, limit) do result[#result] = nil end
    return result
end

--- 【表情检索】【公开结果】返回原版字段，索引路径及待删除资料不进入候选结果
--- @param args table 查询、标签和数量
--- @param ctx table 原始整数上下文
--- @param config table 设置
--- @return table 查询结果
function M.run(args, ctx, config)
    local name, result = paths.selected(args, config), sai.json.array()
    local limit = math.max(1, values.integer(ctx, "limit", 6, 20))
    for _, candidate in ipairs(M.rank(config, name, values.string(args.query), values.strings(args.tags), limit)) do
        local item = candidate.loaded.item
        result[#result + 1] = {id=item.id, name=item.name, score=candidate.score, description=item.description,
            usage=item.usage, avoid=item.avoid, tags=item.tags, animated=item.animated, source=candidate.loaded.source}
    end
    return {success=true, library=name, results=result}
end

return M
