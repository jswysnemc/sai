local M = {}

--- 【知识库快照】【完整读取】只接受完整普通文件，缺失返回 nil
--- @param path string 授权路径
--- @param maximum integer 原始字节上限
--- @return userdata|nil 缓冲句柄
function M.load(path, maximum)
    local info = sai.fs.stat(path)
    if not info then return nil end
    assert(info.is_file, "knowledge base index is not a file: " .. path)
    return sai.binary.read_file(path, {max_bytes=maximum})
end

--- 【知识库快照】【资源作用域】业务结束或异常都关闭快照
--- @param path string 输入路径
--- @param maximum integer 字节上限
--- @param callback function 接收可选缓冲的纯读取回调
--- @return any 回调结果
function M.with(path, maximum, callback)
    local buffer = M.load(path, maximum)
    local result = table.pack(pcall(callback, buffer))
    if buffer then buffer:close() end
    if not result[1] then error(result[2], 0) end
    return table.unpack(result, 2, result.n)
end

--- 【知识库快照】【变更准备】先完成独立事务副本，原索引保持不变
--- @param path string 索引路径
--- @param maximum integer 固定数据库容量
--- @param changes table 结构化变更
--- @return userdata 新快照
--- @return string|nil 原文件摘要
function M.prepare(path, maximum, changes)
    return M.with(path, maximum, function(original)
        local digest = original and original:sha256() or nil
        return M.apply(original, maximum, changes), digest
    end)
end

--- 【知识库快照】【容量估算】按现有镜像与变更预留页空间，小操作不分配整个配置上限
--- @param original userdata|nil 原镜像
--- @param maximum integer 数据库硬上限
--- @param changes table 结构化变更
--- @return userdata 独立事务快照，超过容量时不修改输入
function M.apply(original, maximum, changes)
    local current = original and original:len() or 0
    local growth = #sai.json.encode(changes) * 2 + #changes * 8192
    local capacity = math.min(maximum, math.ceil((current + math.max(65536, growth)) / 4096) * 4096)
    return sai.sqlite.apply(original, changes, {max_bytes=capacity})
end

--- 【知识库快照】【条件提交】对比原修订后发布，异常也关闭输出缓冲
--- @param path string 输出路径
--- @param buffer userdata 新快照
--- @param expected string|nil 旧摘要或缺失条件
--- @return nil 冲突或失败时明确报错
function M.publish(path, buffer, expected)
    local ok, saved = pcall(function() return buffer:write_if(path, expected) end)
    buffer:close()
    if not ok then error(saved, 0) end
    assert(saved, "knowledge base index changed concurrently; retry the operation")
end

--- 【知识库快照】【单表变更】一次变更从当前快照计算并完整发布
--- @param path string 索引路径
--- @param maximum integer 数据库容量
--- @param changes table 结构化变更
--- @return nil 成功时不返回值
function M.update(path, maximum, changes)
    local buffer, expected = M.prepare(path, maximum, changes)
    M.publish(path, buffer, expected)
end

--- 【知识库快照】【结构补齐】存在的空文件或缺表数据库仍可初始化，已有完整结构不重写
--- @param path string 索引路径
--- @param maximum integer 数据库容量
--- @param changes table 幂等建表和建索引请求
--- @return nil 完整结构可用，非法原文件保持不变
function M.ensure(path, maximum, changes)
    local buffer, expected = M.with(path, maximum, function(original)
        local revision = original and original:sha256() or nil
        local input = original and original:len() > 0 and original or nil
        return M.apply(input, maximum, changes), revision
    end)
    if buffer:sha256() == expected then buffer:close()
    else M.publish(path, buffer, expected) end
end

--- 【知识库快照】【完整单页】只对结果总量超限缩页，保持偏移直到完整查询成功
--- @param buffer userdata 输入镜像
--- @param query table 当前偏移和页大小，缩页结果用于后续查询
--- @return table 完整行数组，单行超限和其他失败继续向外传播
local function page(buffer, query)
    while true do
        local ok, result = pcall(sai.sqlite.query, buffer, query)
        if ok then return result.rows end
        if query.limit == 1 or not tostring(result):find("SQLite query result exceeds output limit", 1, true) then
            error(result, 0)
        end
        query.limit = math.max(1, math.floor(query.limit / 2))
    end
end

--- 【知识库快照】【有界遍历】逐页消费完整结果，宽字段触发缩页时不遗漏或重复行
--- @param buffer userdata 输入镜像
--- @param query table 查询字段，不得在遍历中修改
--- @param callback function 消费单行的回调
--- @return nil 遍历完成
function M.each(buffer, query, callback)
    if not buffer then return end
    local offset = 0
    query.limit = query.limit or 32
    while true do
        query.offset = offset
        local rows = page(buffer, query)
        for _, row in ipairs(rows) do callback(row) end
        if #rows < query.limit then return end
        offset = offset + #rows
    end
end

return M
