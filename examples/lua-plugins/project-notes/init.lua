local inspect = require("inspect")
local history = require("history")
local inspections = 0

--- 【项目笔记】【事件观察】统计当前实例中宿主确认成功的笔记工具调用
---@param event SaiToolResultEvent 真实工具名称、成功标志和有界输出
---@param ctx SaiToolContext 只读事件上下文
---@return nil 观察事件不改变宿主结果
local function on_result(event, ctx)
    if event.name == "lua__" .. sai.plugin_id .. "__inspect" and event.ok then
        inspections = inspections + 1
    end
end

--- 【项目笔记】【实例统计】展示当前实例观察到的工具调用数量
---@param arguments string 命令参数，必须为空
---@param ctx SaiContext 当前会话上下文
---@return table 计数及宿主会话标识
local function stats(arguments, ctx)
    assert(sai.text.trim(arguments) == "", "stats takes no arguments")
    return { inspections = inspections, session_id = ctx.session_id }
end

-- 1. 【项目笔记】【统一注册】工具与命令共用业务，写入命令显式声明访问类型
sai.register_tool({
    name = "inspect",
    description = "Read the configured project note and return its SHA-256 and a short preview.",
    parameters = { type = "object", properties = {}, additionalProperties = false },
    access = "read_only",
    execute = inspect.run,
})
sai.register_command({
    name = "inspect", description = "Inspect the configured project note.",
    access = "read_only", execute = inspect.command,
})
sai.register_command({
    name = "remember", description = "Save the latest inspection in private plugin storage.",
    access = "writes", execute = history.remember,
})
sai.register_command({
    name = "latest", description = "Show the saved inspection across sessions.",
    access = "read_only", execute = history.latest,
})
sai.register_command({
    name = "forget", description = "Delete the saved inspection.",
    access = "writes", execute = history.forget,
})
sai.register_command({
    name = "stats", description = "Show successful tool inspections in this instance.",
    access = "read_only", execute = stats,
})
sai.on("tool_result", on_result)
