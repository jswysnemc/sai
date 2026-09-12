local M = {}

--- 【知识库索引】【元数据结构】保留原表名、字段和主键
--- @return table 幂等建表请求
function M.meta()
    return {{op="create_table", name="files", primary_key="name", columns={
        {name="name", kind="text"}, {name="path", kind="text", nullable=false},
        {name="size_bytes", kind="integer", nullable=false}, {name="mtime", kind="real", nullable=false},
        {name="content_sha256", kind="text", nullable=false}, {name="updated_at", kind="real", nullable=false},
    }}}
end

--- 【知识库索引】【语义结构】保留自动编号及文件摘要索引
--- @return table 幂等建表和建索引请求
function M.semantic()
    return {{op="create_table", name="semantic_chunks", primary_key="id", auto_increment=true, columns={
        {name="id", kind="integer"}, {name="provider_id", kind="text", nullable=false},
        {name="model", kind="text", nullable=false}, {name="file_name", kind="text", nullable=false},
        {name="content_sha256", kind="text", nullable=false}, {name="chunk_index", kind="integer", nullable=false},
        {name="start_char", kind="integer", nullable=false}, {name="end_char", kind="integer", nullable=false},
        {name="text", kind="text", nullable=false}, {name="embedding_json", kind="text", nullable=false},
        {name="created_at", kind="real", nullable=false},
    }}, {op="create_index", name="idx_semantic_file", table="semantic_chunks", columns={"file_name", "content_sha256"}}}
end

return M
