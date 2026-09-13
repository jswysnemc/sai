local common = require("providers.common")
local M = {}

--- 【游戏信号】【置信度】保留原证据覆盖、缺失原因与继续调查条件
--- @param app_id integer|nil Steam App ID
--- @param protondb any 成功取得的 JSON 值，nil 表示未取得
--- @param can_i_play string|nil 可玩性页面原文
--- @param anticheat string|nil 反作弊页面原文
--- @param verdict table 可玩性判定
--- @return table 置信度、跟进原因、来源覆盖与建议查询
function M.evaluate(app_id, protondb, can_i_play, anticheat, verdict)
    local has_id, has_protondb = app_id ~= nil, protondb ~= nil
    local has_can_i_play, has_anticheat = can_i_play ~= nil, anticheat ~= nil
    local tier = common.field(protondb, "tier")
    local works = can_i_play and can_i_play:find("Works", 1, true) ~= nil
    local partial = can_i_play and can_i_play:find("Partial", 1, true) ~= nil
    local insufficient = verdict.reason:find("insufficient", 1, true) ~= nil
    local reasons = {}
    if not has_id then reasons[#reasons + 1] = "Steam app id was not found" end
    if not has_protondb then reasons[#reasons + 1] = "ProtonDB data is missing" end
    if not has_can_i_play then reasons[#reasons + 1] = "Can I Play on Linux data is missing" end
    if not has_anticheat then reasons[#reasons + 1] = "AreWeAntiCheatYet data is missing" end
    if insufficient then reasons[#reasons + 1] = "compatibility data is insufficient" end
    local level = "low"
    if has_id and (tier == "platinum" or tier == "gold") and works and has_anticheat then
        level = "high"
    elseif tier == "platinum" or tier == "gold" or tier == "silver" or tier == "bronze" or partial or works then
        level = "medium"
    end
    return {
        level = level,
        needs_followup = level == "low" or insufficient or #reasons > 0,
        followup_reason = #reasons > 0 and table.concat(reasons, "; ") or sai.json.null,
        source_coverage = {
            steam_appid = has_id, protondb = has_protondb,
            can_i_play_on_linux = has_can_i_play, are_we_anticheat_yet = has_anticheat,
        },
        suggested_followup_queries = sai.json.array({
            "ProtonDB game compatibility latest reports",
            "PCGamingWiki Linux Proton known issues",
            "Steam Community Linux Proton performance issues",
        }),
    }
end

return M
