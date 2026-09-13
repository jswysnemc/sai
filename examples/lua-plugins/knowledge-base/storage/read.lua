local snapshot = require("storage.snapshot")
local records = require("storage.records")
local values = require("values")
local paths = require("paths")
local M = {}

--- 【知识库读取】【完整文字】拒绝截断的 UTF-8 文件；索引路径不能把读取引向库外
--- @param config table 配置
--- @param name string 相对路径
--- @return string 完整文字
function M.content(config, name)
    local rel = paths.relative(name)
    return snapshot.with(paths.file(config, rel), config.max_file_bytes, function(buffer)
        assert(buffer, "knowledge base file not found: " .. rel)
        return sai.encoding.to_utf8(buffer:bytes(0, buffer:len()))
    end)
end

--- 【知识库读取】【分页输出】保留原行号、越界提示及继续阅读说明
--- @param config table 配置
--- @param name string 相对文件名
--- @param first string 完整起始行号
--- @param maximum string|nil 可选最大行数
--- @return string 原格式阅读结果
function M.page(config, name, first, maximum)
    assert(records.available(config), "knowledge base is not initialized")
    local rel = paths.relative(name)
    local lines = values.lines(M.content(config, rel))
    if values.exceeds(first, math.max(1, #lines)) then
        return "=== " .. rel .. " | start_line " .. first .. " out of range / " .. #lines .. " lines ==="
    end
    local start = math.max(1, tonumber(first))
    local last = math.min(#lines, start + values.limit(maximum, config.max_read_lines, 5000) - 1)
    local selected = {}
    for index = start, last do selected[#selected + 1] = lines[index] end
    local result = string.format("=== %s | lines %d-%d / %d ===\n%s", rel, start, last, #lines, table.concat(selected, "\n"))
    if last < #lines then result = result .. string.format("\n\n... %d more lines; continue with start_line=%d", #lines - last, last + 1) end
    return result
end

return M
