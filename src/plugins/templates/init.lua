local calls, turns = 0, 0

--- 【示例插件】【问候】返回插件配置和当前会话信息
--- @param args table 可选 name 字段
--- @param ctx table 宿主提供的 session_id、workdir 和 progress
--- @return table 问候文本、会话标识和调用次数
local function greet(args, ctx)
    calls = calls + 1
    ctx.progress("Preparing greeting")
    return {
        text = (sai.config.greeting or "Hello") .. ", " .. (args.name or "world"),
        session_id = ctx.session_id,
        workdir = ctx.workdir,
        calls = calls,
    }
end

--- 【示例插件】【统计】读取当前插件实例的计数
--- @param args string 用户命令参数
--- @param ctx table 当前会话上下文
--- @return table 当前实例的工具调用和模型轮次数量
local function stats(args, ctx)
    return { calls = calls, turns = turns, session_id = ctx.session_id }
end

--- 【示例插件】【轮次观察】记录宿主确认的模型轮次开始事件
--- @param event table 本次生命周期事件
--- @param ctx table 只读会话上下文
local function on_turn(event, ctx)
    turns = turns + 1
end

sai.register_tool({
    name = "greet",
    description = "Return a greeting and the current session context.",
    parameters = {
        type = "object",
        properties = { name = { type = "string" } },
        additionalProperties = false,
    },
    execute = greet,
})
sai.register_command({ name = "stats", description = "Show this plugin instance's counters.", execute = stats })
sai.on("turn_start", on_turn)
