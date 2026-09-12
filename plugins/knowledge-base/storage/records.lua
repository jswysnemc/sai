local snapshot = require("storage.snapshot")
local schema = require("storage.schema")
local paths = require("paths")
local M = {}

--- 【知识库记录】【索引路径】保留原两个数据库文件名
--- @param config table 配置
--- @param semantic boolean 是否选择语义索引
--- @return string 数据库路径
function M.path(config, semantic) return config.data_dir .. (semantic and "/semantic_index.db" or "/kb_meta.db") end

--- 【知识库记录】【初始化】只由有写入许可的管理或变更入口调用
--- @param config table 配置
--- @return nil 目录和两个索引准备完成
function M.init(config)
    sai.fs.create_dir(config.files_dir)
    for _, semantic in ipairs({false, true}) do
        local path = M.path(config, semantic)
        snapshot.ensure(path, config.index_max_bytes, semantic and schema.semantic() or schema.meta())
    end
end

--- 【知识库记录】【可用状态】只读入口不创建库目录或索引
--- @param config table 配置
--- @return boolean 是否存在原知识库的基础结构
function M.available(config)
    local root, files, meta = sai.fs.stat(config.data_dir), sai.fs.stat(config.files_dir), sai.fs.stat(M.path(config, false))
    return root ~= nil and root.is_dir and files ~= nil and files.is_dir and meta ~= nil and meta.is_file
end

--- 【知识库记录】【稳定列表】沿用名称排序，拒绝索引中可越界的相对名称
--- @param config table 配置
--- @return table 文件记录数组
function M.list(config)
    return snapshot.with(M.path(config, false), config.index_max_bytes, function(buffer)
        local records = sai.json.array()
        snapshot.each(buffer, {table="files", columns={"name", "path", "size_bytes", "content_sha256"}, order_by={{column="name"}}}, function(row)
            assert(type(row.name) == "string" and paths.relative(row.name) == row.name, "invalid knowledge base record name")
            assert(type(row.size_bytes) == "number" and row.size_bytes >= 0 and type(row.content_sha256) == "string", "invalid knowledge base record")
            records[#records + 1] = row
        end)
        return records
    end)
end

--- 【知识库记录】【单文件查找】读取当前元数据供后台提交复核
--- @param config table 配置
--- @param name string 已规范化相对路径
--- @return table|nil 当前记录
function M.get(config, name)
    return snapshot.with(M.path(config, false), config.index_max_bytes, function(buffer)
        if not buffer then return nil end
        return sai.sqlite.query(buffer, {table="files", columns={"name", "content_sha256"}, where={name=name}, limit=1}).rows[1]
    end)
end

return M
