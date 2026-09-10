local ddg = require("providers.duckduckgo")
local bing = require("providers.bing")
local ranking = require("ranking")
local M = {}

--- 【网页搜图】【引擎编排】主来源失败或数量不足时回退，再去重并稳定排序
--- @param config table 包设置
--- @param query string 用户查询
--- @param count integer 最终请求数量
--- @param safe boolean 是否启用安全搜索
--- @return table 有界候选数组
function M.run(config, query, count, safe)
    local limit = ranking.pool_limit(count)
    local ok, candidates = pcall(ddg.search, config, query, limit, safe)
    if not ok then candidates = sai.json.array() end
    if #candidates < count then
        local success, fallback = pcall(bing.search, config, query, limit, safe)
        if success then
            for _, item in ipairs(fallback) do candidates[#candidates + 1] = item end
        end
    end
    candidates = ranking.rank(query, ranking.dedupe(candidates))
    assert(#candidates > 0, "image search returned no results")
    while #candidates > limit do table.remove(candidates) end
    return candidates
end

return M
