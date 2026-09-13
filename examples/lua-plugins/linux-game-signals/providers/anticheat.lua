local common = require("providers.common")
local text = require("text")
local M = {}

--- 【游戏信号】【反作弊页面】按候选名称读取 AreWeAntiCheatYet 页面
--- @param slugs table 已排序的路径候选
--- @return table 原始正文、来源及尝试记录
function M.fetch(slugs)
    return common.page(slugs, "https://areweanticheatyet.com/game/", "")
end

--- 【游戏信号】【反作弊摘要】按原优先顺序提取状态、引擎名称和页面摘录
--- @param html string 原始 HTML
--- @return table 摘要，未识别状态时保留 JSON null
function M.summary(html)
    local value = sai.text.html_to_text(html, 120)
    local status = sai.json.null
    for _, candidate in ipairs({ "Supported", "Running", "Planned", "Broken", "Denied" }) do
        if value:find(candidate, 1, true) then
            status = candidate
            break
        end
    end
    return {
        status = status,
        mentions_eac = value:find("Easy Anti-Cheat", 1, true) ~= nil,
        mentions_battleye = value:find("BattlEye", 1, true) ~= nil,
        text_excerpt = text.excerpt(value, 1600),
    }
end

return M
