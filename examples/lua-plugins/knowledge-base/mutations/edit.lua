local read = require("storage.read")
local values = require("values")
local paths = require("paths")
local transaction = require("storage.transaction")
local import = require("mutations.import")
local M = {}

--- 【知识库编辑】【包含末行的替换】保留原末尾换行与 CRLF 处理，空结果按原文件校验拒绝
--- @param config table 配置
--- @param name string 相对文件名
--- @param first string 原起始整数
--- @param last string 原结束整数
--- @param replacement string 替换正文
--- @return table 原编辑结果结构
function M.run(config, name, first, last, replacement)
    return transaction.with(config, true, function()
        name = paths.relative(name)
        assert(first ~= "0" and last ~= "0", "line numbers must be 1-based")
        assert(not (#first > #last or (#first == #last and first > last)), "start_line must be less than or equal to end_line")
        local original = read.content(config, name)
        local lines = values.lines(original)
        assert(not values.exceeds(first, #lines) and not values.exceeds(last, #lines), "line range " .. first .. "-" .. last .. " out of range: " .. #lines .. " lines")
        local added = values.lines(replacement:gsub("\r\n", "\n"):gsub("\r", "\n"))
        local result = {}
        for index = 1, tonumber(first) - 1 do result[#result + 1] = lines[index] end
        for _, line in ipairs(added) do result[#result + 1] = line end
        for index = tonumber(last) + 1, #lines do result[#result + 1] = lines[index] end
        local updated = table.concat(result, "\n")
        if original:sub(-1) == "\n" and updated ~= "" then updated = updated .. "\n" end
        import.content(config, name, updated)
        return {ok=true, path=name, old_line_count=#lines, new_line_count=#result, semantic_refreshed=config.embedding_enabled, warning=sai.json.null}
    end)
end

return M
