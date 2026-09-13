local transaction = require("storage.transaction")
local records = require("storage.records")
local keyword = require("search.keyword")
local semantic = require("search.semantic")
local client = require("embedding.client")
local M = {}

--- 【知识库搜索】【完整检索】网络期间释放数据锁，再以当前文件和索引重新合并
--- @param config table 配置
--- @param query string 查询原文
--- @param limit integer 结果上限
--- @param initialize boolean 管理入口是否允许初始化及恢复
--- @return table 兼容原公开格式的检索结果
function M.search(config, query, limit, initialize)
    local results, allow_semantic = transaction.with(config, initialize, function()
        if not records.available(config) then return sai.json.array(), false end
        return keyword.search(config, query, limit), sai.fs.stat(records.path(config, true)) ~= nil
    end)
    local used = false
    if allow_semantic and config.embedding_enabled and (results[1] and results[1].score or 0) < require("values").f32(config.keyword_strong_score_threshold) then
        local ok, merged, found = pcall(function()
            local provider = client.provider(config)
            if not provider then return results, false end
            local embedding = client.embed(config, provider, query)
            return transaction.with(config, false, function()
                local current = keyword.search(config, query, limit)
                local candidates = semantic.search(config, embedding)
                return semantic.merge(current, candidates, limit), #candidates > 0
            end)
        end)
        if ok then results, used = merged, found end
    end
    return {ok=true, query=query, total_matches=#results, semantic_used=used, results=keyword.present(results)}
end

return M
