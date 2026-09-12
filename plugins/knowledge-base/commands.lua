local transaction = require("storage.transaction")
local records = require("storage.records")
local snapshot = require("storage.snapshot")
local values = require("values")
local arguments = require("command_args")
local M = {}

--- 【知识库命令】【统计】读取原两个索引，保留原 JSON 字段和大小舍入
--- @param config table 配置
--- @return table 原统计格式
local function stats(config)
    return transaction.with(config, true, function()
        local files, size, count = records.list(config), 0, 0
        for _, file in ipairs(files) do size = size + file.size_bytes end
        snapshot.with(records.path(config, true), config.index_max_bytes, function(buffer)
            snapshot.each(buffer, {table="semantic_chunks", columns={"id"}, limit=512}, function() count = count + 1 end)
        end)
        return {ok=true, root=config.data_dir, files_dir=config.files_dir, files=#files,
            total_size_kb=math.floor(size / 1024 * 10 + 0.5) / 10, semantic_chunks=count,
            embedding_enabled=config.embedding_enabled, embedding_provider_id=config.embedding_provider_id, embedding_model=config.embedding_model}
    end)
end

--- 【知识库命令】【命令执行】保留原管理输出，文件和索引逻辑由各模块组合
--- @param config table 配置
--- @param name string 注册命令名
--- @param args table 已解析参数
--- @param ctx table 可信上下文
--- @return table|string 管理输出
local function execute(config, name, args, ctx)
    local zh = config.language == "zh"
    if name == "add" then
        local files = require("mutations.source").add(config, values.required(args, "path"), args.name)
        require("embedding.jobs").enqueue(config)
        if args.format == "json" then return files end
        local lines = {}
        for _, file in ipairs(files) do lines[#lines + 1] = (zh and "已添加 " or "added ") .. file end
        return table.concat(lines, "\n")
    elseif name == "list" or name == "reindex" then
        local files = transaction.with(config, true, function() return records.list(config) end)
        if name == "list" and args.format == "json" then return files end
        if name == "reindex" then return (zh and "关键词索引会按需重建；已跟踪文件数" or "keyword index is rebuilt on demand; files tracked") .. ": " .. #files end
        local lines = {}
        for _, file in ipairs(files) do lines[#lines + 1] = file.name .. "\t" .. file.size_bytes .. (zh and " 字节" or " bytes") end
        return table.concat(lines, "\n")
    elseif name == "search" then
        return require("search.run").search(config, args.query or "", values.limit(values.integer(args, ctx, "limit"), config.max_search_results, 50), true)
    elseif name == "find" then
        return transaction.with(config, true, function() return require("search.keyword").find(config, args.query or "", values.limit(values.integer(args, ctx, "limit"), config.max_search_results, 50)) end)
    elseif name == "read" then
        return transaction.with(config, true, function() return require("storage.read").page(config, values.required(args, "file"), values.integer(args, ctx, "start", "1"), values.integer(args, ctx, "lines")) end)
    elseif name == "remove" then
        local file = values.required(args, "file")
        require("mutations.remove").run(config, file)
        return (zh and "已移除 " or "removed ") .. file
    elseif name == "stats" then return stats(config)
    elseif name == "embed-reindex" then return require("embedding.reindex").run(config, args, ctx) end
    error("unknown knowledge base command")
end

--- 【知识库命令】【完整注册】旧管理查询会初始化目录，因此命令明确声明写入权限
--- @param config table 配置
--- @return nil 九个用户命令完成注册
function M.register(config)
    for _, name in ipairs({"add", "list", "search", "find", "read", "remove", "reindex", "stats", "embed-reindex"}) do
        sai.register_command({name=name, description="Knowledge base " .. name, access="writes",
            execute=function(text, ctx) return execute(config, name, arguments.parse(name, text), ctx) end})
    end
end

return M
