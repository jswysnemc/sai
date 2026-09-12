local values = require("values")
local transaction = require("storage.transaction")
local M = {}

--- 【知识库工具】【入口分派】从原 JSON 取得完整整数，各工具继续保留独立读写声明
--- @param config table 配置
--- @param name string 工具名
--- @param args table 原参数
--- @param ctx table 可信上下文
--- @return table|string 原公开结果
local function execute(config, name, args, ctx)
    if name == "search_knowledge_base" then
        return require("search.run").search(config, values.required(args, "query"), values.limit(values.integer(args, ctx, "max_results"), config.max_search_results, 50), false)
    elseif name == "search_knowledge_base_by_name" then
        local query = values.required(args, "file_name_query")
        return transaction.with(config, false, function() return require("search.keyword").find(config, query, values.limit(values.integer(args, ctx, "max_results"), config.max_search_results, 50)) end)
    elseif name == "read_knowledge_base_file" then
        local file = values.required(args, "file_name")
        return transaction.with(config, false, function() return require("storage.read").page(config, file, values.integer(args, ctx, "start_line", "1"), values.integer(args, ctx, "max_lines")) end)
    elseif name == "upload_text_to_knowledge_base" then
        local result = require("mutations.import").upload(config, args)
        require("embedding.jobs").enqueue(config)
        return result
    elseif name == "edit_knowledge_base_file" then
        local file = values.required(args, "file_name")
        local first = assert(values.integer(args, ctx, "start_line"), "start_line is required")
        local last = assert(values.integer(args, ctx, "end_line"), "end_line is required")
        assert(type(args.replacement) == "string", "replacement is required")
        local result = require("mutations.edit").run(config, file, first, last, args.replacement)
        require("embedding.jobs").enqueue(config)
        return result
    elseif name == "remove_knowledge_base_file" then
        return require("mutations.remove").run(config, values.required(args, "file_name"))
    end
    error("unknown knowledge base tool")
end

--- 【知识库工具】【契约注册】直接发布原六个名称，上传开关控制三项写入工具
--- @param config table 实例配置
--- @return nil 所有可用工具完成注册
function M.register(config)
    for _, definition in ipairs(require("definitions")) do
        if not definition.writes or config.upload_tool_enabled then
            local name = definition.name
            sai.register_tool({name=name, description=definition.description,
                parameters=definition.parameters, access=definition.writes and "writes" or "read_only",
                execute=function(args, ctx) return execute(config, name, args, ctx) end})
        end
    end
end

return M
