local M = {}
local key = "knowledge:queue"

--- 【知识库后台】【队列记录】只有小型协调信息进入插件私有状态，正文仍保留在知识库
--- @return table|nil 当前队列序号和等待状态
local function current()
    local state = sai.storage.plugin.get(key)
    if state == nil or state == sai.json.null then return nil end
    assert(type(state) == "table" and math.type(state.sequence) == "integer" and state.sequence > 0
        and type(state.waiting) == "boolean" and type(state.configuration) == "string",
        "invalid knowledge base queue state")
    return state
end

--- 【知识库后台】【配置归属】重建条件改变时不合并其他目录或供应商的等待任务
--- @param config table 已解析配置
--- @return string 配置摘要，不把明文凭据写入记录
local function identity(config)
    local provider = config.provider or {}
    return sai.crypto.digest("sha256", sai.json.encode(sai.json.array({
        sai.fs.realpath(config.data_dir), config.embedding_provider_id, config.embedding_model,
        config.semantic_chunk_chars, config.semantic_chunk_overlap, config.max_file_bytes,
        config.index_max_bytes, provider.endpoint or "", provider.api_key or "", provider.api_key_env or "",
    })))
end

--- 【知识库后台】【活动等待者】调度器 Running 也可能正在等嵌入锁，使用队列序号识别同一任务
--- @param state table 当前等待记录
--- @return boolean 是否仍有对应活动任务
local function waiting(state)
    if not state.waiting then return false end
    for offset = 0, 112, 16 do
        local tasks = sai.scheduler.list({offset=offset, limit=16})
        for _, task in ipairs(tasks) do
            if task.command == "embed-reindex" and (task.status == "scheduled" or task.status == "running") then
                local ok, arguments = pcall(sai.json.decode, task.arguments)
                if ok and type(arguments) == "table" and arguments.ticket == tostring(state.sequence) then return true end
            end
        end
        if #tasks < 16 then return false end
    end
    return false
end

--- 【知识库后台】【重建调度】合并还未取得嵌入锁的任务，实际扫描开始后允许一项后续任务
--- @param config table 配置
--- @return nil 调度失败明确返回错误，已发布正文仍保留
function M.enqueue(config)
    if not config.embedding_enabled or sai.text.trim(config.embedding_provider_id) == "" or sai.text.trim(config.embedding_model) == "" then return end
    sai.storage.plugin.with_lock("knowledge:jobs", function()
        local state, configuration = current(), identity(config)
        if state and state.configuration == configuration and waiting(state) then return end
        local sequence = state and state.sequence or 0
        assert(sequence < math.maxinteger, "knowledge base queue sequence exhausted")
        state = {sequence=sequence + 1, waiting=true, configuration=configuration}
        -- 1. 【知识库后台】【先存序号】取消发生在任务发布前后时，下次通过实际活动任务判断是否复用
        sai.storage.plugin.set(key, state)
        sai.scheduler.schedule({due_at=sai.time.now(), command="embed-reindex",
            arguments=sai.json.encode({quiet=true, background=true, ticket=tostring(state.sequence)})})
    end)
end

--- 【知识库后台】【开始扫描】取得嵌入锁之后才清除等待标记，不清除较晚任务的序号
--- @param ticket string|nil 当前任务序号，普通手动重建不带序号
--- @return nil 后续编辑可以为真正运行中的扫描再安排一次任务
function M.started(ticket)
    if not ticket then return end
    sai.storage.plugin.with_lock("knowledge:jobs", function()
        local state = current()
        if state and tostring(state.sequence) == ticket then
            state.waiting = false
            sai.storage.plugin.set(key, state)
        end
    end)
end

return M
