local rows = sai.json.array()
local saved_progress

--- 【生命周期验收】【事件记录】保存宿主事实并尝试事件阶段禁止的能力
---@param kind string 注册的生命周期事件名称
---@param event table 宿主提供的事件数据
---@param ctx table 只读调用上下文
---@return table|nil 工具拒绝原因，观察事件返回 nil
local function observe(kind, event, ctx)
    if (kind == "tool_call" or kind == "tool_result") and event.name ~= "lifecycle_probe" then
        return nil
    end
    -- 1. 【生命周期验收】【权限边界】清单和用户已授权，事件仍不能取得调用或写入服务
    local model_allowed = pcall(sai.model.complete, {
        messages = { { role = "user", content = "listener must not send this request" } },
    })
    local tools_allowed = pcall(sai.tools.list)
    local session_write_allowed = pcall(sai.storage.set, "event-marker", { written = true })
    local plugin_write_allowed = pcall(sai.storage.plugin.set, "event-marker", { written = true })
    rows[#rows + 1] = {
        kind = kind,
        data = sai.json.decode(sai.json.encode(event)),
        session_id = ctx.session_id,
        operation_id = ctx.operation_id,
        workdir = ctx.workdir,
        allow_writes = ctx.allow_writes,
        model_allowed = model_allowed,
        tools_allowed = tools_allowed,
        session_write_allowed = session_write_allowed,
        plugin_write_allowed = plugin_write_allowed,
    }
    saved_progress = ctx.progress
    -- 2. 【生命周期验收】【参数隔离】监听器可拒绝调用，但不能改写宿主实际执行参数
    if kind == "tool_call" then
        if event.arguments.deny then return { deny = "observer denied probe" } end
        event.arguments.value = "listener attempted replacement"
    end
    return nil
end

for _, kind in ipairs({
    "agent_start", "agent_end", "turn_start", "turn_end",
    "message_start", "message_end", "tool_call", "tool_result",
}) do
    sai.on(kind, function(event, ctx) return observe(kind, event, ctx) end)
end

--- 【生命周期验收】【读取快照】通过独立命令读取同一实例的事件与持久化检查结果
---@param arguments string 未使用的命令参数
---@param ctx table 本次命令上下文
---@return table 事件数组、过期回调状态和两类存储内容
local function report(arguments, ctx)
    local old_progress_allowed = false
    if saved_progress then old_progress_allowed = pcall(saved_progress, "expired callback") end
    return {
        rows = rows,
        old_progress_allowed = old_progress_allowed,
        session_record = sai.storage.get("event-marker") or sai.json.null,
        plugin_record = sai.storage.plugin.get("event-marker") or sai.json.null,
    }
end

sai.register_command({
    name = "report",
    description = "Read observed lifecycle facts.",
    access = "read_only",
    execute = report,
})
