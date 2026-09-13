local schema = require("index_schema")
local values = require("values")
local M = {max_bytes=1048576, attempts=16}

--- 【表情索引】【完整快照】从同一有界字节缓冲取得正文和摘要，拒绝截断及非法 UTF-8
--- @param path string 授权内 JSON 文件
--- @return any|nil JSON 对象；缺失返回 nil
--- @return string|nil 原始完整字节 SHA-256
function M.read_json(path)
    local stat = sai.fs.stat(path)
    if not stat then return nil, nil end
    assert(stat.is_file, "meme JSON path is not a file: " .. path)
    local buffer = sai.binary.read_file(path, {max_bytes=M.max_bytes})
    local ok, data, digest = pcall(function()
        return sai.json.decode(buffer:bytes(0, buffer:len())), buffer:sha256()
    end)
    buffer:close()
    if not ok then error(data, 0) end
    return data, digest
end

--- 【表情索引】【加载】校验存在的索引，缺失时只在内存中构造空覆盖层
--- @param path string 索引文件
--- @param library string 所选库名
--- @return table 索引对象
--- @return string|nil 条件写入摘要
function M.load(path, library)
    local data, digest = M.read_json(path)
    if data == nil then data = {library=library, version=2} end
    return schema.validate(data), digest
end

--- 【表情索引】【条件发布】目标未变化时完整替换，失败和冲突不会部分覆盖
--- @param path string 索引或状态文件
--- @param data table JSON 对象
--- @param expected string|nil 原字节摘要或缺失条件
--- @return boolean 是否发布成功
function M.publish(path, data, expected)
    local text = sai.json.encode(data)
    assert(#text <= M.max_bytes, "meme JSON exceeds 1 MiB")
    local buffer = sai.binary.from_bytes(text)
    local ok, changed = pcall(function() return buffer:write_if(path, expected) end)
    buffer:close()
    if not ok then error(changed, 0) end
    return changed
end

--- 【表情索引】【并发修改】每次冲突重新读取并合并，回调不能执行外部写入
--- @param path string 用户索引
--- @param library string 所选库名
--- @param change function 基于当前索引返回业务结果及是否需要保存
--- @return any 成功发布对应的业务结果
function M.mutate(path, library, change)
    for _ = 1, M.attempts do
        local current, expected = M.load(path, library)
        local result, changed = change(current)
        if changed == false then return result end
        current.library, current.version = library, 2
        schema.validate(current)
        if M.publish(path, current, expected) then return result end
    end
    error("meme index changed concurrently; retry the operation")
end

--- 【表情索引】【数组移除】保留原顺序及 JSON 数组类型
--- @param items table 原数组
--- @param predicate function 返回 true 表示移除
--- @return table 新数组
function M.without(items, predicate)
    local result = sai.json.array()
    for _, item in ipairs(items) do
        if not predicate(item) then result[#result + 1] = item end
    end
    return result
end

--- 【表情索引】【待删除查找】允许显式删除重试定位尚未完成的记录
--- @param current table 用户索引
--- @param id string 请求标识
--- @return table|nil 待删除记录
function M.pending(current, id)
    for _, pending in ipairs(current.pending_deletions) do
        if values.ids_match(pending.id, id) then return pending end
    end
end

--- 【表情索引】【随机文件标识】生成独占创建使用的后缀，不用时间作为唯一性保证
--- @return string 32 位十六进制后缀
function M.token()
    return string.format("%08x%08x%08x%08x", math.random(0, 0xffffffff), math.random(0, 0xffffffff),
        math.random(0, 0xffffffff), math.random(0, 0xffffffff))
end

return M
