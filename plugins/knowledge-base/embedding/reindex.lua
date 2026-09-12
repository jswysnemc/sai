local client = require("embedding.client")
local chunks = require("embedding.chunks")
local records = require("storage.records")
local transaction = require("storage.transaction")
local snapshot = require("storage.snapshot")
local read = require("storage.read")
local values = require("values")
local M = {}

--- 【知识库嵌入】【向量序列化】选择可还原同一 f32 的短十进制表示
--- @param vector table 有限单精度向量
--- @return string 索引中保存的 JSON 数组
local function encode_vector(vector)
    local parts = {}
    for _, value in ipairs(vector) do
        local text
        for digits = 1, 9 do
            text = string.format("%." .. digits .. "g", value)
            if values.f32(tonumber(text)) == value then break end
        end
        parts[#parts + 1] = text
    end
    return "[" .. table.concat(parts, ",") .. "]"
end

--- 【知识库嵌入】【完整快照替换】在内存中分批构建单文件全部块，只发布最终快照
--- @param config table 配置
--- @param name string 文件名
--- @param rows table 本次成功取得的文本块
--- @return nil 任一步失败保留旧语义索引
local function replace(config, name, rows)
    local path = records.path(config, true)
    local buffer, revision = snapshot.prepare(path, config.index_max_bytes, {{op="delete", table="semantic_chunks", where={file_name=name}}})
    local ok, problem = pcall(function()
        local batch, bytes = {}, 0
        --- 【知识库嵌入】【批次写入】把当前块追加到内存快照并释放上一份副本
        --- @return nil 无参数，更新快照并清空批次，不发布文件
        local function flush()
            if #batch == 0 then return end
            local updated = snapshot.apply(buffer, config.index_max_bytes, {{op="insert", table="semantic_chunks", rows=batch}})
            buffer:close(); buffer = updated
            batch, bytes = {}, 0
        end
        for _, row in ipairs(rows) do
            local size = #sai.json.encode(row)
            assert(size <= 900000, "semantic chunk exceeds database request limit")
            if #batch >= 32 or bytes + size > 900000 then flush() end
            batch[#batch + 1], bytes = row, bytes + size
        end
        flush()
        snapshot.publish(path, buffer, revision)
        buffer = nil
    end)
    if buffer then buffer:close() end
    if not ok then error(problem, 0) end
end

--- 【知识库嵌入】【逐文件重建】网络期间不占数据锁，提交前核对元数据与实际正文
--- @param config table 配置
--- @param provider table 固定供应商
--- @param quiet boolean 是否静默
--- @param ctx table 进度输出上下文
--- @return integer 实际写入块数
local function rebuild(config, provider, quiet, ctx)
    local files = transaction.with(config, true, function() return records.list(config) end)
    local indexed = 0
    for _, record in ipairs(files) do
        local ok, content = pcall(function()
            return transaction.with(config, false, function() return read.content(config, record.name) end)
        end)
        if ok then
            local digest, rows = sai.crypto.digest("sha256", content), {}
            for _, chunk in ipairs(chunks.build(content, config.semantic_chunk_chars, config.semantic_chunk_overlap)) do
                local fetched, embedding = pcall(client.embed, config, provider, chunk.text)
                if fetched then
                    rows[#rows + 1] = {provider_id=provider.id, model=sai.text.trim(config.embedding_model), file_name=record.name,
                        content_sha256=record.content_sha256, chunk_index=chunk.index, start_char=chunk.start, end_char=chunk["end"],
                        text=chunk.text, embedding_json=encode_vector(embedding), created_at=sai.time.utc_now().unix_ms / 1000}
                elseif not quiet then
                    ctx.progress(values.truncate(("embedding failed for %s chunk %d: %s"):format(record.name, chunk.index, tostring(embedding)), 4000))
                end
            end
            transaction.with(config, true, function()
                local current = records.get(config, record.name)
                if not current or current.content_sha256 ~= record.content_sha256 then return end
                local readable, latest = pcall(read.content, config, record.name)
                if not readable or sai.crypto.digest("sha256", latest) ~= digest then return end
                replace(config, record.name, rows)
                indexed = indexed + #rows
            end)
        end
    end
    return indexed
end

--- 【知识库嵌入】【管理与后台入口】独立嵌入锁自动释放，旧锁文件继续阻止重建以兼容旧进程
--- @param config table 配置
--- @param options table quiet 和可选后台等待标志
--- @param ctx table 可信上下文
--- @return string 原命令输出，静默时为空
function M.run(config, options, ctx)
    local quiet = options.quiet == true
    transaction.with(config, true, function() end)
    if not config.embedding_enabled then return quiet and "" or "embedding is disabled" end
    local provider = client.provider(config)
    if not provider then return quiet and "" or "embedding provider/model is not configured; skipped" end
    local legacy_lock = config.data_dir .. "/embedding.lock"
    --- 【知识库嵌入】【占用提示】区分旧锁文件和自动释放的宿主锁
    --- @param legacy boolean 是否发现旧版锁文件
    --- @return string 符合静默选项的占用说明
    local function busy(legacy)
        if not legacy then return quiet and "" or "embedding reindex already running" end
        return quiet and "" or ("embedding reindex already running; lock file: " .. legacy_lock .. "\nif no sai reindex process is running, remove the stale lock file and retry")
    end
    if sai.fs.stat(legacy_lock) then return busy(true) end
    local ok, result = pcall(sai.storage.plugin.with_lock, "knowledge:embedding", function()
        if options.background then require("embedding.jobs").started(options.ticket) end
        return rebuild(config, provider, quiet, ctx)
    end, {timeout_ms=options.background and 600000 or 1})
    if not ok then
        if tostring(result):find("plugin lock acquisition timed out", 1, true) then return busy(false) end
        error(result, 0)
    end
    return quiet and "" or ("indexed semantic chunks: " .. result)
end

return M
