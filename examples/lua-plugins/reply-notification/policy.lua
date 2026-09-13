local settings = require("settings")
local M = {}

local messages = {
    completed={en="Reply complete", zh="答复已完成"},
    interrupted={en="Reply interrupted", zh="答复已中断"},
    failed={en="Reply failed", zh="答复失败"},
}

--- 【答复通知】【正文整理】去除首尾空白和换行，按 Unicode 字符限制通知长度
--- @param text string 原始通知正文
--- @param max_chars integer 包含省略号的字符上限
--- @return string 可直接展示的正文
function M.format_body(text, max_chars)
    local trimmed = sai.text.trim(text):gsub("\n", " ")
    if utf8.len(trimmed) <= max_chars then return trimmed end
    local last = utf8.offset(trimmed, math.max(0, max_chars - 1) + 1)
    return trimmed:sub(1, last - 1) .. "…"
end

--- 【答复通知】【策略计算】为交互式答复结束事件生成通知，不执行平台副作用
--- @param event table 交互面、结束状态与区域代码
--- @return table|nil 通知内容及独立开关，禁用或不适用时返回 nil
function M.plan(event)
    if event.surface ~= "tui" and event.surface ~= "web" then return nil end
    if not settings.enabled and not settings.sound then return nil end
    local message = messages[event.status]
    if message == nil then return nil end
    local language = (event.locale or ""):lower():sub(1, 2) == "zh" and "zh" or "en"
    return {
        title="Sai",
        body=M.format_body(message[language], 240),
        desktop=settings.enabled,
        sound=settings.sound,
    }
end

return M
