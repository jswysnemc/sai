local http = require("http")
local search = require("search")
local reports = require("reports")
local format = require("report_format")

--- 【ProtonDB】【查询】解析游戏并汇总评级和有数量限制的评论
--- @param args table 包含 query 和可选 max_reports
--- @return table 游戏资料、评级、评论及来源
local function query(args)
    local text = sai.text.trim(args.query or "")
    assert(text ~= "", "missing required argument: query")
    local limit = args.max_reports
    if limit == nil or limit < 0 then limit = 10 end
    limit = math.min(limit, 40)

    -- 【ProtonDB】【查询】1. 保留数字查询的名称回退，评级失败时明确报告错误
    local id, name, oslist = search.resolve(text)
    local ok, summary = pcall(http.json, "https://www.protondb.com/api/v1/reports/summaries/" .. id .. ".json")
    assert(ok, "ProtonDB summary fetch failed for app " .. id .. ": " .. tostring(summary))

    -- 【ProtonDB】【查询】2. 评论不可用时保留已取得的游戏资料和评级
    local fetched, data = pcall(reports.fetch, id)
    if not fetched or type(data) ~= "table" then data = {} end
    local items = sai.json.array()
    for index, report in ipairs(type(data.reports) == "table" and data.reports or {}) do
        if index > limit then break end
        items[#items + 1] = format.extract(report)
    end
    local rating = {}
    for target, source in pairs({
        tier = "tier", confidence = "confidence", score = "score", total = "total",
        best_reported_tier = "bestReportedTier", trending_tier = "trendingTier",
    }) do
        local value
        if type(summary) == "table" then value = summary[source] end
        if value == nil then value = sai.json.null end
        rating[target] = value
    end
    local total = math.type(data.total) == "integer" and data.total >= 0 and data.total or 0
    return {
        app_id = id, game_name = name, oslist = oslist, summary = rating,
        reports = { total = total, returned = #items, items = items },
        protondb_url = "https://www.protondb.com/app/" .. id,
    }
end

sai.register_tool({
    name = "protondb_query",
    description = "Query ProtonDB game compatibility ratings and user reports. Use when you need to check Linux game compatibility, ProtonDB tiers, or read user reports/comments for a game. Accepts either a Steam App ID (numeric) or a game title (text search). / 查询 ProtonDB 游戏兼容性评级和用户评论。在需要查询 Linux 游戏兼容性信息等场景时使用。支持 Steam App ID（数字）或游戏名称（文本搜索）。",
    parameters = {
        type = "object",
        properties = {
            query = { type = "string", description = 'Steam App ID (e.g. "1245620") or game title (e.g. "Elden Ring"). / Steam App ID 或游戏名称。' },
            max_reports = { type = "integer", description = "Maximum number of user reports to return, default 10. / 最多返回的评论数，默认 10。" },
        },
        required = { "query" }, additionalProperties = false,
    },
    access = "read_only",
    execute = query,
})
