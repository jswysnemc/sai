local storage = require("state")
local M = {}

--- 【待办提醒】【工具后判断】每个工具循环最多提醒一次，成功修改会重置连续计数
--- @param input table 宿主工具名称、原参数、结果和当前可见的本插件工具
--- @param previous table|nil 当前循环上次返回的状态
--- @param ctx table 可信只读上下文
--- @return table|nil 下一次计数状态及可选提醒
function M.after_tool(input, previous, ctx)
    local enabled = false
    for _, name in ipairs(input.tools) do if name == "todo" then enabled = true end end
    if not enabled then return nil end
    local state = type(previous) == "table" and previous or {unchanged=0, injected=false}
    local action = type(input.arguments) == "table" and input.arguments.action or nil
    local updated = (input.local_name == "todo" or input.name == "todo") and input.ok
        and (action == "add" or action == "update" or action == "remove")
    if updated or not storage.has_unfinished() then
        state.unchanged = 0
        return {state=state}
    end
    state.unchanged = state.unchanged + 1
    if state.injected or state.unchanged < 3 then return {state=state} end
    state.injected = true
    return {state=state, reminder="<system-reminder>当前会话仍有未完成 TODO，且连续 " .. state.unchanged
        .. " 个工具轮没有更新清单。请使用 todo 工具核对并更新进度后继续。</system-reminder>"}
end

--- 【待办提醒】【回复准备】待办只使用工具后策略，不增加主请求前的上下文
--- @param input string 当前用户消息
--- @param ctx table 可信调用上下文
--- @return nil 无准备资料
function M.prepare(input, ctx) return nil end

--- 【待办提醒】【回复完成】待办不安排回复后的外部动作
--- @param delivery table 原准备资料
--- @param ctx table 可信调用上下文
--- @return nil 无完成资料
function M.complete(delivery, ctx) return nil end

return M
