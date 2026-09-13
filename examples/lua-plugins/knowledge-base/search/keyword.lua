local tokens = require("search.tokens")
local snippets = require("search.snippets")
local values = require("values")
local paths = require("paths")
local read = require("storage.read")
local records = require("storage.records")
local f32 = values.f32
local M = {}

--- 【知识库搜索】【关键词评分】沿用短语、文件名、命中数量和邻近覆盖评分
--- @param config table 配置
--- @param query string 原查询
--- @param limit integer 结果上限
--- @return table 按原顺序稳定排序的候选
function M.search(config, query, limit)
    local words, phrase, results = tokens.query(query), values.lower(query), sai.json.array()
    for _, record in ipairs(records.list(config)) do
        local ok, content = pcall(read.content, config, record.name)
        if ok then
            local lower, name, score = values.lower(content), values.lower(record.name), 0
            local matched, positions, count = {}, {}, 0
            --- 【知识库搜索】【命中去重】每个词元只增加一次覆盖数量
            --- @param token string 已命中的词元
            --- @return nil 更新当前命中集合和数量
            local function mark(token)
                if not matched[token] then matched[token], count = true, count + 1 end
            end
            if #phrase > 1 and lower:find(phrase, 1, true) then score = score + 90; mark(phrase) end
            if #phrase > 1 and name:find(phrase, 1, true) then score = score + 140 end
            for _, token in ipairs(words) do
                local found = tokens.positions(lower, token)
                if #found > 0 then
                    score = f32(score + 20 + math.min(10, #found) * 2)
                    mark(token); positions[token] = found
                end
                if name:find(token, 1, true) then score = f32(score + 45); mark(token) end
            end
            if #words > 0 then score = f32(score + f32(f32(count / #words) * 55)) end
            local window = snippets.best(positions, words, config.proximity_window_chars)
            if window then
                score = f32(score + f32(window.coverage * 120))
                results[#results + 1] = {path=record.name, score=score, source="keyword", snippets=sai.json.array({snippets.extract(content, window.start, window["end"], config.snippet_context_chars)})}
            elseif score > 0 then
                results[#results + 1] = {path=record.name, score=score, source="keyword", snippets=snippets.fallback(content, lower, words, config.snippet_context_chars)}
            end
        end
    end
    values.sort(results)
    while #results > limit do table.remove(results) end
    return results
end

--- 【知识库搜索】【名称评分】保留完整路径、文件名和部分词元的原优先级
--- @param query string 查询
--- @param name string 相对文件名
--- @return number 原名称评分
--- @return string 匹配原因
function M.name_score(query, name)
    query, name = values.lower(query:gsub("\\", "/")), values.lower(name:gsub("\\", "/"))
    local base = paths.name(name)
    if query == name then return 1000.0, "exact_path" end
    if query == base then return 950.0, "exact_file_name" end
    if name:find(query, 1, true) then return 820.0 + math.min(60, #query), "path_contains" end
    if base:find(query, 1, true) then return 760.0 + math.min(60, #query), "file_name_contains" end
    local count = 0
    for _, word in ipairs(tokens.query(query)) do if name:find(word, 1, true) then count = count + 1 end end
    if count == 0 then return 0.0, "" end
    return 300.0 + count * 80, "partial_name_terms"
end

--- 【知识库搜索】【文件名检索】名称与大小来自原元数据表
--- @param config table 配置
--- @param query string 查询
--- @param limit integer 结果上限
--- @return table 原公开 JSON 结构
function M.find(config, query, limit)
    local results = sai.json.array()
    if records.available(config) then
        for _, record in ipairs(records.list(config)) do
            local score, reason = M.name_score(query, record.name)
            if score > 0 then results[#results + 1] = {path=record.name, name=paths.name(record.name), directory=paths.directory(record.name), score=score,
                match_reason=reason, size_kb=math.floor(record.size_bytes / 1024 * 10 + 0.5) / 10} end
        end
        values.sort(results)
        while #results > limit do table.remove(results) end
    end
    return {ok=true, query=query, total_matches=#results, results=results}
end

--- 【知识库搜索】【结果展示】保留原单精度评分与路径输出
--- @param results table 内部候选数组
--- @return table 可序列化结果数组
function M.present(results)
    for _, result in ipairs(results) do
        result.name, result.directory, result.score = paths.name(result.path), paths.directory(result.path), values.score(result.score)
    end
    return results
end

return M
