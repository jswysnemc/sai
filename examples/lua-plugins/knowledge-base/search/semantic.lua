local values = require("values")
local snapshot = require("storage.snapshot")
local records = require("storage.records")
local f32 = values.f32
local M = {}

--- 【知识库语义】【余弦评分】逐项保留原 f32 乘加与平方根精度
--- @param left table 查询向量
--- @param right table 索引向量
--- @return number 余弦相似度，空值或维度不同返回零
function M.cosine(left, right)
    if #left ~= #right or #left == 0 then return 0.0 end
    local dot, a, b = 0, 0, 0
    for index, value in ipairs(left) do
        value = f32(value)
        local other = f32(right[index])
        dot, a, b = f32(dot + f32(value * other)), f32(a + f32(value * value)), f32(b + f32(other * other))
    end
    if a <= 0 or b <= 0 then return 0.0 end
    return f32(dot / f32(f32(math.sqrt(a)) * f32(math.sqrt(b))))
end

--- 【知识库语义】【索引向量】无效 JSON 或非数字数组与原版一样跳过
--- @param text string JSON 向量
--- @return table|nil 有效单精度向量
local function vector(text)
    local ok, data = pcall(sai.json.decode, text)
    if not ok or type(data) ~= "table" or getmetatable(data) ~= getmetatable(sai.json.array()) then return nil end
    for index, value in ipairs(data) do
        if type(value) ~= "number" then return nil end
        value = f32(value)
        if value ~= value or math.abs(value) == math.huge then return nil end
        data[index] = value
    end
    return data
end

--- 【知识库语义】【快照搜索】遍历既有块并保持原相似度阈值与 top-k
--- @param config table 配置
--- @param embedding table 查询向量
--- @return table 排序后的语义候选
function M.search(config, embedding)
    return snapshot.with(records.path(config, true), config.index_max_bytes, function(buffer)
        local results = sai.json.array()
        snapshot.each(buffer, {table="semantic_chunks", columns={"file_name", "text", "embedding_json"}, order_by={{column="id"}}, limit=32}, function(row)
            local stored = vector(row.embedding_json)
            if stored then
                local score = M.cosine(embedding, stored)
                if score == score and math.abs(score) < math.huge and score >= f32(config.semantic_min_score) then
                    results[#results + 1] = {path=row.file_name, score=f32(score * 200), source="semantic", snippets=sai.json.array({sai.text.collapse_whitespace(row.text)})}
                end
            end
        end)
        values.sort(results)
        while #results > config.semantic_top_k do table.remove(results) end
        return results
    end)
end

--- 【知识库语义】【结果合并】同文件增加 0.6 倍语义分并限制片段数
--- @param results table 关键词结果
--- @param semantic table 语义结果
--- @param limit integer 最终结果上限
--- @return table 合并后的结果
function M.merge(results, semantic, limit)
    for _, item in ipairs(semantic) do
        local found
        for _, existing in ipairs(results) do if existing.path == item.path then found = existing; break end end
        if found then
            found.score = f32(found.score + f32(item.score * f32(0.6)))
            for _, snippet in ipairs(item.snippets) do if #found.snippets < 4 then found.snippets[#found.snippets + 1] = snippet end end
        else results[#results + 1] = item end
    end
    values.sort(results)
    while #results > limit do table.remove(results) end
    return results
end

return M
