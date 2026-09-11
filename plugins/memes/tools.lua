local definitions = require("definitions")
local descriptions = require("descriptions")
local values = require("values")
local M = {}
local handlers = {
    search_meme=require("search").run, show_meme=require("display").run,
    add_meme=require("mutations.add").run, update_meme=require("mutations.update").run,
    delete_meme=require("mutations.delete").run,
}

--- 【表情工具】【说明本地化】递归替换原工具说明，参数键和值的类型保持不变
--- @param definition table 工具或 JSON Schema 对象
--- @return nil 原地更新说明
local function translate(definition)
    for key, value in pairs(definition) do
        if key == "description" and type(value) == "string" then
            definition[key] = descriptions[value] or value
        elseif type(value) == "table" then translate(value) end
    end
end

--- 【表情工具】【最近记录】仅查询旧状态文件，不触发自动发送
--- @param args table 空参数对象
--- @param ctx table 可信上下文
--- @param config table 插件配置
--- @return table 最近一次自动发送结果
function handlers.recent_meme(args, ctx, config) return require("reply_state").recent(config) end

--- 【表情工具】【完整注册】六个名称和参数来自原公开契约，读写权限分别声明
--- @param config table 本实例设置
--- @return nil 六工具完成注册
function M.register(config)
    for _, definition in ipairs(values.copy(definitions)) do
        if config.language == "zh" then translate(definition) end
        local handler = assert(handlers[definition.name])
        sai.register_tool({
            name=definition.name, description=definition.description, parameters=definition.parameters,
            access=definition.writes and "writes" or "read_only",
            execute=function(args, ctx) return handler(args, ctx, config) end,
        })
    end
end

return M
