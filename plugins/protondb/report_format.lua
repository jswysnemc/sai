local M = {}
local null = sai.json.null
local array_metatable = getmetatable(sai.json.array())
local fault_types = {
    { "audioFaults", "audio" }, { "graphicalFaults", "graphics" },
    { "windowingFaults", "windowing" }, { "inputFaults", "input" },
    { "saveGameFaults", "save_game" }, { "performanceFaults", "performance" },
    { "stabilityFaults", "stability" }, { "significantBugs", "significant_bugs" },
}

--- 【ProtonDB】【对象字段】安全读取可选的 JSON 对象
--- @param value any 原始字段
--- @return table 对象或空表
local function object(value)
    return type(value) == "table" and value or {}
end

--- 【ProtonDB】【非负整数】提取评论计数、时长或日期
--- @param value any 原始字段
--- @return integer 有效整数或零
local function unsigned(value)
    return math.type(value) == "integer" and value >= 0 and value or 0
end

--- 【ProtonDB】【可选文本】去除文本两端空白并保留 JSON null
--- @param value any 原始文本字段
--- @return string|userdata 非空文本或 JSON null
local function trimmed(value)
    if type(value) ~= "string" then return null end
    local text = sai.text.trim(value)
    return text ~= "" and text or null
end

--- 【ProtonDB】【故障字段】提取启用的故障类型、详情和说明
--- @param responses table 用户问卷结果
--- @param notes table 用户故障备注
--- @return table 故障数组，详情键保持稳定排序
local function faults(responses, notes)
    local result = sai.json.array()
    local follow_up = object(responses.followUp)
    for _, field in ipairs(fault_types) do
        local key, label = field[1], field[2]
        if responses[key] == "yes" then
            local details = sai.json.array()
            local value = follow_up[key]
            if type(value) == "table" and getmetatable(value) ~= array_metatable then
                for name in pairs(value) do details[#details + 1] = name end
                table.sort(details)
            elseif type(value) == "string" then
                details[1] = value
            end
            result[#result + 1] = { type = label, details = details, note = trimmed(notes[key]) }
        end
    end
    return result
end

--- 【ProtonDB】【评论格式】整理推荐状态、版本、日期、故障和启动参数
--- @param report table 原始用户评论
--- @return table 兼容原工具的评论条目
function M.extract(report)
    report = object(report)
    local contributor = object(object(report.contributor).steam)
    local responses = object(report.responses)
    local notes = object(responses.notes)
    local playtime = unsigned(contributor.playtime)
    local timestamp = unsigned(report.timestamp)
    local date = timestamp > 0 and sai.time.iso(timestamp) or nil
    local recommended = "broken"
    if responses.startsPlay == "yes" then
        local verdict = type(responses.verdictOob) == "string" and responses.verdictOob
            or (type(responses.verdict) == "string" and responses.verdict or nil)
        recommended = verdict and (verdict == "yes" and "recommended" or "not_recommended") or "unknown"
    end
    local version = responses.protonVersion
    if responses.variant == "experimental" then
        version = "Proton Experimental"
    elseif responses.variant == "ge" then
        version = responses.customProtonVersion
    end
    local concluding = type(notes.concludingNotes) == "string" and notes.concludingNotes
        or responses.concludingNotes
    return {
        author = type(contributor.nickname) == "string" and contributor.nickname or "anonymous",
        report_count = unsigned(contributor.reportTally),
        playtime_hours = playtime > 0 and playtime // 60 or null,
        date = date and date:sub(1, 10) or "unknown",
        recommended = recommended,
        proton_version = type(version) == "string" and version or null,
        launch_options = trimmed(responses.launchOptions),
        faults = faults(responses, notes), notes = trimmed(concluding),
    }
end

return M
