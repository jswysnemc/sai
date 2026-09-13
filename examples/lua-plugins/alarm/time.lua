local M = {}
local MAX_TIME = 253402300799

--- 【闹钟】【整数解析】只接受十进制无符号整数和可选加号，先限制范围再计算
--- @param value string 数字文本
--- @param maximum integer 允许上限
--- @return integer 已校验整数
local function unsigned(value, maximum)
    assert(value:match("^%+?%d+$"), "invalid alarm time number")
    value = value:gsub("^%+", ""):gsub("^0+", "")
    if value == "" then return 0 end
    assert(#value <= 12, "alarm time is outside the supported range")
    local number = tonumber(value)
    assert(number and number <= maximum, "alarm time is outside the supported range")
    return number
end

--- 【闹钟】【时钟时间】按当前本地时钟计算下一次目标分钟，过去时间跨到次日
--- @param value string 时钟文本
--- @param now integer 当前 Unix 秒
--- @return integer 到期前秒数
local function clock_seconds(value, now)
    local hour, minute = value:match("^(%+?%d+):(%+?%d+)$")
    assert(hour and minute, "invalid clock time")
    hour, minute = unsigned(hour, 23), unsigned(minute, 59)
    local h, m, s = sai.time.local_format("%H %M %S", now):match("^(%d+) (%d+) (%d+)$")
    assert(h and m and s, "invalid local clock result")
    local seconds = hour * 3600 + minute * 60 - (tonumber(h) * 3600 + tonumber(m) * 60 + tonumber(s))
    if seconds <= 0 then seconds = seconds + 86400 end
    return seconds
end

--- 【闹钟】【到期计算】解析组合时长或时钟时间，完整限制整数运算与最终时间范围
--- @param value string 原始时间文本
--- @param now integer 当前 Unix 秒
--- @return integer 绝对到期时间
function M.due_at(value, now)
    assert(type(value) == "string" and #value <= 256, "invalid alarm time text")
    assert(math.type(now) == "integer" and now >= 0 and now <= MAX_TIME, "invalid current time")
    local text = sai.text.collapse_whitespace(value)
    assert(text ~= "", "time is required")
    local parts = {}
    for part in text:gmatch("%S+") do parts[#parts + 1] = part end
    -- 1. 【闹钟】【时钟分支】单个含冒号的参数按原有时钟语义解析
    if #parts == 1 and parts[1]:find(":", 1, true) then
        local due = now + clock_seconds(parts[1], now)
        assert(due <= MAX_TIME, "alarm time is outside the supported range")
        return due
    end
    -- 2. 【闹钟】【时长分支】逐段检查乘法和累计上限，拒绝溢出及多字节未知单位
    local total = 0
    local units = {h=3600, m=60, s=1}
    for _, part in ipairs(parts) do
        assert(#part >= 2, "invalid alarm time")
        local multiplier = units[part:sub(-1):lower()]
        assert(multiplier, "invalid alarm time unit")
        local amount = unsigned(part:sub(1, -2), math.floor((MAX_TIME - now - total) / multiplier))
        total = total + amount * multiplier
    end
    assert(total > 0, "alarm time must be greater than zero")
    return now + total
end

return M
