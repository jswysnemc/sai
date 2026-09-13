local common = require("providers.common")
local text = require("text")
local M = {}

--- 【游戏信号】【兼容性判定】按原优先级分析来源及联机诉求，交通灯使用文本值
--- @param protondb any 已取得的原始评级，nil 表示缺失
--- @param can_i_play string|nil 可玩性页面原文
--- @param anticheat string|nil 反作弊页面原文
--- @param issue string 用户关注点
--- @return table 交通灯、中文结论和依据
function M.evaluate(protondb, can_i_play, anticheat, issue)
    local lower = text.ascii_lower(issue)
    local multiplayer = lower:find("multi", 1, true) or lower:find("online", 1, true)
        or issue:find("联机", 1, true) or issue:find("多人", 1, true) or issue:find("反作弊", 1, true)
    local denied = anticheat and (anticheat:find("Denied", 1, true) or anticheat:find("Broken", 1, true))
    if multiplayer and denied then
        return { traffic_light = "red", label = "不可玩", reason = "anti-cheat denied or broken for multiplayer/online use" }
    end
    if can_i_play and can_i_play:find("Broken", 1, true) then
        return { traffic_light = "red", label = "不可玩", reason = "Can I Play on Linux marks it broken" }
    end
    local tier = common.field(protondb, "tier")
    if tier == "platinum" or tier == "gold" or (can_i_play and can_i_play:find("Works", 1, true)) then
        return { traffic_light = "green", label = "可玩", reason = "ProtonDB/Can I Play on Linux indicate it works" }
    end
    if tier == "silver" or tier == "bronze" or (can_i_play and can_i_play:find("Partial", 1, true)) then
        return { traffic_light = "yellow", label = "不一定能玩", reason = "partial or lower confidence compatibility" }
    end
    return { traffic_light = "yellow", label = "不一定能玩", reason = "insufficient compatibility data" }
end

return M
