local M = {}

--- 【知识库配置】【整数校验】拒绝非法配置，避免溢出或悄悄改变范围
--- @param config table 合并配置
--- @param key string 字段
--- @param minimum integer 最小值
--- @param maximum integer 最大值
--- @return nil 校验失败抛出错误
local function integer(config, key, minimum, maximum)
    local value = config[key]
    assert(type(value) == "number" and value == math.floor(value)
        and value >= minimum and value <= maximum, "knowledge-base." .. key .. " is out of range")
end

--- 【知识库配置】【实例快照】旧字段由宿主定向提供，Lua 只解析本包设置
--- @param input table 当前包设置
--- @return table 已校验且独立的配置
function M.resolve(input)
    local config = sai.json.decode(sai.json.encode(require("defaults")))
    for key, value in pairs(input) do
        assert(config[key] ~= nil or key == "provider" or key == "provider_error", "unknown knowledge-base setting: " .. tostring(key))
        config[key] = value
    end
    for _, key in ipairs({"data_dir", "allowed_extensions", "allowed_filenames", "embedding_provider_id", "embedding_model"}) do
        assert(type(config[key]) == "string", "knowledge-base." .. key .. " must be a string")
    end
    assert(config.data_dir ~= "", "knowledge-base.data_dir must not be empty")
    assert(config.language == "en" or config.language == "zh", "knowledge-base.language must be en or zh")
    for _, key in ipairs({"upload_tool_enabled", "embedding_enabled"}) do
        assert(type(config[key]) == "boolean", "knowledge-base." .. key .. " must be a boolean")
    end
    for _, key in ipairs({"max_search_results", "max_read_lines", "semantic_top_k", "embedding_timeout_seconds"}) do
        integer(config, key, 1, math.maxinteger)
    end
    integer(config, "max_file_size_kb", 1, 4096)
    integer(config, "index_max_bytes", 65536, 8388608)
    integer(config, "semantic_chunk_chars", 128, 65536)
    integer(config, "semantic_chunk_overlap", 0, config.semantic_chunk_chars - 1)
    integer(config, "snippet_context_chars", 0, 1048576)
    integer(config, "proximity_window_chars", 0, math.maxinteger)
    for _, key in ipairs({"semantic_min_score", "keyword_strong_score_threshold"}) do
        assert(type(config[key]) == "number" and config[key] == config[key] and math.abs(config[key]) < math.huge,
            "knowledge-base." .. key .. " must be finite")
    end
    config.max_file_bytes = config.max_file_size_kb * 1024
    config.files_dir = config.data_dir:gsub("/$", "") .. "/files"
    return config
end

return M
