local common = require("providers.common")
local steam = require("providers.steam")
local protondb = require("providers.protondb")
local can_i_play = require("providers.can_i_play")
local anticheat = require("providers.anticheat")
local query = require("query")
local verdict = require("verdict")
local confidence = require("confidence")
local M = {}

--- 【游戏信号】【完整采集】按原来源顺序采集证据，部分来源失败时保留已取得结果
--- @param args table 游戏名称与可选关注点
--- @return table 匹配信息、尝试记录、兼容性结论、置信度和来源摘要
function M.run(args)
    -- 1. 【游戏信号】【输入准备】只接受有效游戏名称，构造原查询候选
    local game = sai.text.trim(args.game or "")
    assert(game ~= "", "missing required argument: game")
    local issue = sai.text.trim(args.issue or "")
    local candidates = query.candidates(game)
    local matched, attempts = steam.fetch(candidates)
    local app_id = common.field(matched, "appid")
    if math.type(app_id) ~= "integer" or app_id < 0 then app_id = nil end
    local matched_name = common.field(matched, "name")
    if type(matched_name) ~= "string" then matched_name = game end
    local slugs = query.slugs(candidates, matched_name, app_id ~= nil)

    -- 2. 【游戏信号】【来源读取】按 Steam、ProtonDB、可玩页面、反作弊页面顺序执行
    local rating = protondb.fetch(app_id)
    local playable = can_i_play.fetch(slugs)
    local anti = anticheat.fetch(slugs)

    -- 3. 【游戏信号】【证据分析】原始页面参与判定，摘要使用有界 HTML 转文本
    local decision = verdict.evaluate(rating, playable.text, anti.text, issue)
    local certainty = confidence.evaluate(app_id, rating, playable.text, anti.text, decision)
    local playable_summary, anti_summary
    if playable.text ~= nil then playable_summary = can_i_play.summary(playable.text) end
    if anti.text ~= nil then anti_summary = anticheat.summary(anti.text) end

    -- 4. 【游戏信号】【结果组装】缺失来源保留 null，已取得的空正文或 JSON null 保留覆盖状态
    return {
        ok = true, game_query = game, search_query = candidates[1] or game,
        query_candidates = candidates, matched_name = matched_name,
        steam = common.optional(matched),
        source_attempts = {
            steam = attempts, can_i_play_on_linux = playable.attempts, are_we_anticheat_yet = anti.attempts,
        },
        verdict = decision, confidence = certainty, needs_followup = certainty.needs_followup,
        protondb = common.optional(rating),
        can_i_play_on_linux = common.optional(playable_summary),
        are_we_anticheat_yet = common.optional(anti_summary),
        sources = {
            steam = app_id and ("https://store.steampowered.com/app/" .. app_id .. "/") or sai.json.null,
            protondb = app_id and ("https://www.protondb.com/app/" .. app_id) or sai.json.null,
            can_i_play_on_linux = common.optional(playable.url),
            are_we_anticheat_yet = common.optional(anti.url),
        },
        methodology = "If ProtonDB exists, use ProtonDB reports/comments as the primary practical playability signal. If ProtonDB is missing or insufficient, continue with web_search/web_fetch outside this tool. Keep final answer concise and include 调查结果, 依据, 怎么玩, 注意事项.",
    }
end

return M
