local format = {}
local array = sai.json.array
local null = sai.json.null
local severity = { full_outage = 4, partial_outage = 3, degraded = 2, maintenance = 1 }

--- 【服务状态】【类型归一】将可选 JSON 对象或数组归一为可遍历表
--- @param value any JSON 字段
--- @return table 原表或空表
local function object(value)
    return type(value) == "table" and value or {}
end

--- 【服务状态】【文本字段】按类型读取字符串并应用缺省值
--- @param record table JSON 对象
--- @param key string 字段名称
--- @param fallback string|nil 缺省文本
--- @return string 文本字段
local function text(record, key, fallback)
    return type(record[key]) == "string" and record[key] or (fallback or "")
end

--- 【服务状态】【时间格式】将秒时间戳转为东八区 ISO 时间
--- @param seconds any 可选秒时间戳
--- @return string|userdata ISO 时间或 JSON null
local function iso(seconds)
    if type(seconds) ~= "number" then return null end
    return sai.time.iso(math.modf(seconds), 28800) or null
end

--- 【服务状态】【事件格式】归一近期事件及其时间线
--- @param change table 原始事件对象
--- @return table 稳定的事件输出结构
local function incident(change)
    local started, resolved = change.start_at_seconds, change.close_at_seconds
    local affected, timeline = array(), array()
    for _, item in ipairs(object(change.affected_components)) do
        affected[#affected + 1] = text(item, "name", text(item, "component_name"))
    end
    for _, item in ipairs(object(change.updates)) do
        timeline[#timeline + 1] = { time = iso(item.at_seconds), status = text(item, "status"), description = text(item, "description") }
    end
    local duration = null
    if type(started) == "number" and type(resolved) == "number" then duration = math.modf(resolved - started) end
    return {
        id = change.change_id or null, title = text(change, "title"),
        type = text(change, "type", "incident"), status = text(change, "status"),
        started_at = iso(started), resolved_at = iso(resolved), duration_seconds = duration,
        affected_components = affected, timeline = timeline,
    }
end

--- 【服务状态】【结果汇总】把服务状态、可用率和近期事件组装成原有工具契约
--- @param raw table 页面解析结果
--- @param include_incidents boolean 是否返回近期事件
--- @param limit number 近期事件数量上限
--- @return table 可交给模型的 JSON 结构
function format.response(raw, include_incidents, limit)
    local config = object(raw.initialPageConfig)
    local active = object(raw.active_changes)
    local uptimes = object(raw.component_uptimes)
    local components, recent = array(), array()
    -- 1. 组合组件与可用率，并叠加当前故障状态
    for _, item in ipairs(object(config.components)) do
        local id, uptime, status = text(item, "component_id"), null, "operational"
        for _, entry in ipairs(uptimes) do
            if text(entry, "component_id") == id and entry.uptime ~= nil then uptime = entry.uptime; break end
        end
        for _, change in ipairs(active) do
            for _, affected in ipairs(object(change.affected_components)) do
                if text(affected, "component_id") == id then status = text(change, "status", "unknown") end
            end
        end
        components[#components + 1] = {
            id = id, name = text(item, "name"), description = text(item, "description"),
            status = status, uptime_30d_percent = uptime,
        }
    end
    -- 2. 按严重程度汇总状态，近期事件保留页面顺序
    local status, readable = "operational", "All Systems Operational"
    if #active > 0 then
        local rank = -1
        for _, change in ipairs(active) do
            local current = text(change, "status", "unknown")
            if (severity[current] or 0) >= rank then status, rank = current, severity[current] or 0 end
        end
        readable = status:gsub("_", " "):gsub("(%a)([%w]*)", function(first, rest) return first:upper() .. rest end)
            .. " - " .. #active .. " active incident(s)"
    end
    if include_incidents then
        for _, change in ipairs(object(object(raw.initialCalendarData).changes)) do
            if #recent >= limit then break end
            recent[#recent + 1] = incident(change)
        end
    end
    return {
        success = true, queried_at = iso(sai.time.now()), overall_status = status,
        status_readable = readable, active_incidents_count = #active,
        components = components, recent_incidents = recent,
    }
end

return format
