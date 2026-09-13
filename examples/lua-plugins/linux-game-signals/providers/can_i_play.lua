local common = require("providers.common")
local text = require("text")
local M = {}

--- 【游戏信号】【可玩页面】按候选名称读取 Can I Play on Linux 页面
--- @param slugs table 已排序的路径候选
--- @return table 原始正文、来源及尝试记录
function M.fetch(slugs)
    return common.page(slugs, "https://caniplayonlinux.com/games/", "/")
end

--- 【游戏信号】【可玩摘要】保留页面状态、来源标注的 Proton 版本和章节摘录
--- @param html string 原始 HTML
--- @return table 页面摘要，缺失章节与标签保留 JSON null
function M.summary(html)
    local value = sai.text.html_to_text(html, 120)
    return {
        works = value:find("Works", 1, true) ~= nil,
        partial = value:find("Partial", 1, true) ~= nil,
        broken = value:find("Broken", 1, true) ~= nil,
        source_recommended_proton = common.optional(text.after_label(value, "Recommended Proton")),
        steam_deck_verified = value:find("Steam Deck Verified", 1, true) ~= nil,
        known_issues = common.optional(text.section(value, "Known issues", "Fixes", 1200)),
        fixes = common.optional(text.section(value, "Fixes", "Verdict", 1200)),
        text_excerpt = text.excerpt(value, 2000),
    }
end

return M
