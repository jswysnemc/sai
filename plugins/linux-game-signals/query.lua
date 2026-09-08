local text = require("text")
local M = {}

--- 【游戏信号】【查询归一化】复用原中文别名规则，保留未知游戏名称
--- @param game string 用户游戏名称
--- @return string 归一化后的查询
function M.normalize(game)
    local compact = text.ascii_lower(sai.text.collapse_whitespace(game):gsub(" ", ""))
    for _, alias in ipairs({ "赛博朋克2077", "电驭叛客2077", "cyberpunk2077" }) do
        if compact:find(alias, 1, true) then return "Cyberpunk 2077" end
    end
    if compact:find("原神", 1, true) or compact:find("genshinimpact", 1, true) then
        return "Genshin Impact"
    end
    return sai.text.trim(game)
end

--- 【游戏信号】【名称候选】生成原有单项查询列表，空文本不形成候选
--- @param game string 用户游戏名称
--- @return table 按原规则生成的 JSON 数组
function M.candidates(game)
    local value = M.normalize(game)
    return value ~= "" and sai.json.array({ value }) or sai.json.array()
end

--- 【游戏信号】【路径名称】将 ASCII 字母数字以外的连续字符转换为连字符
--- @param value string 游戏名称
--- @return string 去掉首尾连字符的路径名称
function M.slug(value)
    return (text.ascii_lower(value):gsub("[^a-z0-9]+", "-"):gsub("^-", ""):gsub("-$", ""))
end

--- 【游戏信号】【路径候选】合并原查询与已匹配名称，再排序并去重
--- @param candidates table 查询名称数组
--- @param matched_name string Steam 返回的有效名称
--- @param has_app_id boolean 是否存在有效的非负整数 App ID
--- @return table 排序后的路径数组，已匹配名称允许生成空路径以保持原行为
function M.slugs(candidates, matched_name, has_app_id)
    local seen, result = {}, sai.json.array()
    for _, candidate in ipairs(candidates) do
        local slug = M.slug(candidate)
        if slug ~= "" then seen[slug] = true end
    end
    if has_app_id then seen[M.slug(matched_name)] = true end
    for slug in pairs(seen) do result[#result + 1] = slug end
    table.sort(result)
    return result
end

return M
