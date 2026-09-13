local http = require("http")
local format = require("status_format")
local M = {}
local base = "https://status.archlinux.org"
local page_id = "vmM5ruWEAB"

--- 【Arch 状态】【接口查询】读取固定状态页面下的接口
--- @param endpoint string 接口名称
--- @param query string|nil 可选查询串
--- @return table 接口 JSON
local function fetch(endpoint, query)
    return http.json(base .. "/api/" .. endpoint .. "/" .. page_id .. (query or ""),
        "sai-arch-status/0.1", 10000)
end

--- 【Arch 状态】【服务查询】汇总事件、AUR 监控与最近停机
--- @param args table 空参数对象
--- @return table 兼容原工具的服务状态
function M.query(args)
    -- 【Arch 状态】【服务查询】1. 读取事件和监控列表，优先定位 AUR
    local event_data = fetch("getEventFeed")
    local monitor_data = fetch("getMonitorList")
    local events = type(event_data.results) == "table" and event_data.results or {}
    local monitors = type(monitor_data.data) == "table" and monitor_data.data or {}
    local latest_event = events[1] ~= nil and format.event(events[1]) or nil
    local aur = format.monitor(monitors) or {}
    local monitor_id = math.type(aur.monitorId) == "integer" and aur.monitorId >= 0 and aur.monitorId or nil

    -- 【Arch 状态】【服务查询】2. 可选详情失败时保留列表状态与事件信息
    local detail = {}
    if monitor_id then
        local ok, response = pcall(fetch, "getMonitor", "?m=" .. monitor_id)
        if ok and type(response) == "table" then
            detail = type(response.monitor) == "table" and response.monitor or {}
        end
    end
    local status = type(detail.statusClass) == "string" and detail.statusClass
        or (type(aur.statusClass) == "string" and aur.statusClass or "")
    local is_degraded, reason = format.degraded(status, latest_event)
    return {
        success = true, current_state = format.current_state(status),
        is_degraded = is_degraded, degraded_reason = reason,
        latest_down = format.latest_down(type(detail.logs) == "table" and detail.logs or {}),
        latest_event = latest_event or sai.json.null,
        monitor = {
            name = type(aur.name) == "string" and aur.name or "AUR",
            status_class = status, monitor_id = monitor_id or sai.json.null,
        },
        source = base,
    }
end

return M
