local M = {}

--- 【哈希工具】【结果文本】按原版字段及算法插入顺序输出 JSON，避免 Lua 表顺序影响结果
--- @param byte_length integer 原始输入字节数
--- @param names table 去重后的算法名称，保持首次出现顺序
--- @param values table 每个原始名称对应的最终摘要或错误文本
--- @return string 与原版一致的缩进 JSON
function M.hashes(byte_length, names, values)
    local rows = {}
    for _, name in ipairs(names) do
        rows[#rows + 1] = "    " .. sai.json.encode(name) .. ": " .. sai.json.encode(values[name])
    end
    local results = #rows == 0 and "{}" or ("{\n" .. table.concat(rows, ",\n") .. "\n  }")
    return '{\n  "success": true,\n  "byte_length": ' .. tostring(byte_length)
        .. ',\n  "results": ' .. results .. "\n}"
end

--- 【文本解码】【结果文本】保持原版字段顺序、缩进和 JSON 字符串转义
--- @param text string 解码后的 UTF-8 文本
--- @return string 与原版一致的缩进 JSON
function M.decoded(text)
    return '{\n  "success": true,\n  "decoded_text": ' .. sai.json.encode(text) .. "\n}"
end

return M
