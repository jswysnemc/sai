local snapshot = require("storage.snapshot")
local records = require("storage.records")
local paths = require("paths")
local M = {}
local journal_name = "/pending-write.json"

--- 【知识库事务】【文件摘要】完整读取当前目标，缺失返回 nil
--- @param config table 配置
--- @param path string 已授权文件
--- @return string|nil 完整 SHA-256
function M.digest(config, path)
    return snapshot.with(path, config.max_file_bytes, function(buffer) return buffer and buffer:sha256() or nil end)
end

--- 【知识库事务】【恢复记录】校验完整待提交记录，禁止记录指定任意绝对路径
--- @param config table 配置
--- @return table|nil 待完成变更
local function pending(config)
    return snapshot.with(config.data_dir .. journal_name, sai.limits.output_bytes, function(buffer)
        if not buffer then return nil end
        local value = sai.json.decode(buffer:bytes(0, buffer:len()))
        assert(type(value) == "table" and value.version == 1 and (value.kind == "write" or value.kind == "remove"), "invalid knowledge base pending write")
        assert(type(value.name) == "string" and paths.relative(value.name) == value.name, "invalid knowledge base pending name")
        assert(type(value.clear_semantic) == "boolean", "invalid knowledge base pending index change")
        assert(value.expected == sai.json.null or (type(value.expected) == "string" and value.expected:match("^[a-f0-9]+$") and #value.expected == 64), "invalid knowledge base pending revision")
        if value.kind == "write" then
            assert(type(value.content) == "string", "invalid knowledge base pending text")
            paths.validate(config, value.name, value.content)
        end
        return value
    end)
end

--- 【知识库事务】【索引变更】按最终文件状态生成幂等元数据请求
--- @param config table 配置
--- @param plan table 已校验写入记录
--- @param modified number 修改时间
--- @return table 元数据变更
local function metadata(config, plan, modified)
    if plan.kind == "remove" then return {{op="delete", table="files", where={name=plan.name}}} end
    return {{op="upsert", table="files", key="name", rows={{name=plan.name, path=paths.file(config, plan.name),
        size_bytes=#plan.content, content_sha256=sai.crypto.digest("sha256", plan.content),
        mtime=modified, updated_at=sai.time.utc_now().unix_ms / 1000}}}}
end

--- 【知识库事务】【幂等完成】先验证索引副本，再更新文件，最后发布索引并移除恢复记录
--- @param config table 配置
--- @param plan table 完整待提交记录
--- @return nil 失败保留待完成记录，后续写入入口继续恢复
local function finish(config, plan)
    local path, meta_path, semantic_path = paths.file(config, plan.name), records.path(config, false), records.path(config, true)
    local now = sai.time.utc_now().unix_ms / 1000
    local probe = snapshot.prepare(meta_path, config.index_max_bytes, metadata(config, plan, now))
    probe:close()
    local semantic, semantic_revision
    if plan.clear_semantic then
        semantic, semantic_revision = snapshot.prepare(semantic_path, config.index_max_bytes, {{op="delete", table="semantic_chunks", where={file_name=plan.name}}})
    end
    local ok, problem = pcall(function()
        -- 1. 【知识库事务】【条件文件】重试已写完的内容；拒绝覆盖外部修改后的文件
        if plan.kind == "write" then
            local current, desired = M.digest(config, path), sai.crypto.digest("sha256", plan.content)
            local expected = plan.expected ~= sai.json.null and plan.expected or nil
            if current ~= desired then
                assert(current == expected, "knowledge base file changed during pending write; restore the expected revision before retrying")
                local buffer = sai.binary.from_bytes(plan.content)
                snapshot.publish(path, buffer, expected)
            end
            local info = assert(sai.fs.stat(path), "knowledge base file disappeared after write")
            now = info.modified or now
        else
            local current = M.digest(config, path)
            local expected = plan.expected ~= sai.json.null and plan.expected or nil
            assert(not current or current == expected,
                "knowledge base file changed during pending removal; restore the expected revision before retrying")
            if current then sai.fs.remove_file(path) end
        end
        -- 2. 【知识库事务】【索引提交】同一锁下重新计算元数据，重复恢复不会重复插入文件记录
        snapshot.update(meta_path, config.index_max_bytes, metadata(config, plan, now))
        if semantic then
            snapshot.publish(semantic_path, semantic, semantic_revision)
            semantic = nil
        end
        sai.fs.remove_file(config.data_dir .. journal_name)
    end)
    if semantic then semantic:close() end
    if not ok then error(problem, 0) end
end

--- 【知识库事务】【作用域】所有知识库读取与变更共享插件锁，网络请求不持有数据锁
--- @param config table 配置
--- @param writes boolean 是否已取得写入许可并允许恢复
--- @param callback function 库业务回调
--- @return any 回调结果
function M.with(config, writes, callback)
    return sai.storage.plugin.with_lock("knowledge:store", function()
        local plan = pending(config)
        if writes then
            records.init(config)
            if plan then finish(config, plan) end
        else
            assert(not plan, "knowledge base has an unfinished write; run sai kb reindex to recover")
        end
        return callback()
    end)
end

--- 【知识库事务】【记录后提交】文件变更之前保存完整操作，取消不依赖 Lua 清理回调
--- @param config table 配置
--- @param plan table 包含类型、名称、正文和语义清理标志
--- @return nil 完成变更或保留可恢复记录
function M.commit(config, plan)
    plan.version = 1
    plan.expected = M.digest(config, paths.file(config, plan.name)) or sai.json.null
    local body = sai.json.encode(plan)
    assert(#body <= sai.limits.output_bytes, "knowledge base pending write exceeds output limit")
    snapshot.publish(config.data_dir .. journal_name, sai.binary.from_bytes(body), nil)
    finish(config, plan)
end

return M
