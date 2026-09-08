local M = {}
local null = sai.json.null

--- 【Arch 状态】【对象字段】将可选 JSON 对象转换为可安全访问的表
--- @param value any 接口字段
--- @return table 对象或空表
local function object(value)
    return type(value) == "table" and value or {}
end

--- 【Arch 状态】【字符串字段】读取可选字符串
--- @param value any 接口字段
--- @return string 字符串或空串
local function text(value)
    return type(value) == "string" and value or ""
end

--- 【Arch 状态】【字段兼容】保留显式 null、布尔值和原始字段
--- @param value any 接口字段
--- @return any 原字段或 JSON null
local function nullable(value)
    if value == nil then return null end
    return value
end

--- 【Arch 状态】【影响服务】提取事件正文中列出的服务
--- @param content string 事件正文
--- @return table 服务名称数组
local function affected_services(content)
    local services = sai.json.array()
    local capture = false
    for line in content:gmatch("[^\r\n]+") do
        line = sai.text.trim(line)
        if line ~= "" then
            if line:lower():find("affected services:", 1, true) == 1 then
                capture = true
            elseif capture and line:sub(1, 1) == "-" then
                while line:sub(1, 2) == "- " do line = line:sub(3) end
                line = sai.text.trim(line)
                services[#services + 1] = line
            elseif capture and #services > 0 then
                break
            end
        end
    end
    return services
end

--- 【Arch 状态】【监控选择】优先使用 AUR 名称或域名，兼容列表首项回退
--- @param monitors table 状态监控列表
--- @return table|nil 选中的监控记录
function M.monitor(monitors)
    for _, item in ipairs(monitors) do
        local monitor = object(item)
        if text(monitor.name):lower() == "aur"
            or text(monitor.url):lower():find("aur.archlinux.org", 1, true) then
            return monitor
        end
    end
    return monitors[1] and object(monitors[1]) or nil
end

--- 【Arch 状态】【事件格式】保留事件状态、原始时间和受影响服务
--- @param item table 原始事件
--- @return table 兼容原工具的事件字段
function M.event(item)
    item = object(item)
    local content = item.content
    if content == nil then content = item.description end
    content = text(content)
    local ended_at = item.endDateGMT
    if ended_at == nil then ended_at = item.endDate end
    local started_at = math.type(item.timestamp) == "integer" and sai.time.iso(item.timestamp) or nil
    return {
        title = nullable(item.title), type = nullable(item.type), event_type = nullable(item.eventType),
        is_active = ended_at == nil or ended_at == null,
        started_at = nullable(started_at), started_at_raw = nullable(item.timeGMT),
        ended_at = nullable(ended_at), content = content, status = nullable(item.status),
        affected_services = affected_services(content),
    }
end

--- 【Arch 状态】【当前状态】将监控样式映射为 up、down 或 unknown
--- @param status string 原始监控样式
--- @return string 服务状态
function M.current_state(status)
    status = status:lower()
    if status == "success" then return "up" end
    if status == "danger" or status == "down" or status == "error" then return "down" end
    return "unknown"
end

--- 【Arch 状态】【降级判定】结合未结束事件和监控状态判断 AUR 是否降级
--- @param status string 监控样式
--- @param event table|nil 最近一次归一化事件
--- @return boolean 是否降级
--- @return string|userdata 降级原因或 JSON null
function M.degraded(status, event)
    if event and event.is_active then
        local affected = false
        for _, service in ipairs(event.affected_services) do
            local lower = service:lower()
            if lower == "aur" or lower:find("aur.archlinux.org", 1, true) then affected = true end
        end
        if affected or event.content:lower():find("aur", 1, true)
            or text(event.title):lower():find("aur", 1, true) then
            return true, "Arch status page has an unresolved incident affecting AUR"
        end
    end
    status = status:lower()
    if status == "warning" or status == "degraded" then
        return true, "AUR monitor status is not fully healthy"
    end
    return false, null
end

--- 【Arch 状态】【最近停机】读取日志中第一条停机及其简短原因
--- @param logs table 监控日志
--- @return table|userdata 停机记录或 JSON null
function M.latest_down(logs)
    for _, value in ipairs(logs) do
        local item = object(value)
        if text(item.label):lower() == "down" or text(item.class):lower() == "danger" then
            local reason = object(object(item.reason).detail).short
            local started_at = item.dateGMTISO
            if started_at == nil then started_at = item.timeGMT end
            return {
                started_at = nullable(started_at), duration = nullable(item.duration),
                reason = type(reason) == "string" and reason or null,
            }
        end
    end
    return null
end

return M
