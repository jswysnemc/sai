local M = {}

--- 【诊断文本】【字符前缀】按 Unicode 字符数截取文本
--- @param value string 原文
--- @param count integer 字符上限
--- @return string 不切断 UTF-8 的前缀
function M.prefix(value, count)
    local ending = utf8.offset(value, count + 1)
    return ending and value:sub(1, ending - 1) or value
end

--- 【诊断文本】【摘要截断】保留原版去空白和省略号规则
--- @param value string 原文
--- @param count integer 字符上限
--- @return string 诊断摘要
function M.clip(value, count)
    value = sai.text.trim(value)
    if utf8.len(value) <= count then return value end
    return M.prefix(value, math.max(count - 3, 0)) .. "..."
end

--- 【诊断文本】【行拆分】与 Rust lines 一致处理 CRLF 和末尾换行
--- @param value string 原文
--- @return table 文本行数组
function M.lines(value)
    local result, offset = sai.json.array(), 1
    while offset <= #value do
        local ending = value:find("\n", offset, true)
        local line = value:sub(offset, ending and ending - 1 or #value)
        if ending and line:sub(-1) == "\r" then line = line:sub(1, -2) end
        result[#result + 1] = line
        offset = ending and ending + 1 or #value + 1
    end
    return result
end

--- 【诊断文本】【空白分词】使用统一 Unicode 空白规则拆分字段
--- @param value string 原文
--- @return table 字段数组
function M.words(value)
    local result = sai.json.array()
    for word in sai.text.collapse_whitespace(value):gmatch("%S+") do result[#result + 1] = word end
    return result
end

--- 【诊断文本】【关键字匹配】按字面量匹配任一关键字
--- @param value string 原文
--- @param needles table 关键字列表
--- @return boolean 是否匹配
function M.contains_any(value, needles)
    for _, needle in ipairs(needles) do if value:find(needle, 1, true) then return true end end
    return false
end

--- 【诊断文本】【主目录脱敏】替换主目录的字面量，不解释 Lua 模式或替换表达式
--- @param value string 原文
--- @param home string|nil 当前用户主目录
--- @return string 脱敏结果
function M.redact(value, home)
    if not home or home == "" then return value end
    local result, offset = {}, 1
    while true do
        local first, last = value:find(home, offset, true)
        if not first then result[#result + 1] = value:sub(offset); break end
        result[#result + 1] = value:sub(offset, first - 1) .. "$HOME"
        offset = last + 1
    end
    return table.concat(result)
end

--- 【诊断文本】【命令名称】保留原版名称识别规则，执行入口另行排除选项与特殊名称
--- @param value string 命令名
--- @return boolean 是否只含原版允许的 ASCII 字符
function M.safe_command_name(value)
    return value ~= "" and value:match("^[A-Za-z0-9_.+%-]+$") ~= nil
end

--- 【诊断文本】【执行目标】拒绝选项、路径、shell 表达式和环境赋值
--- @param value string 命令名
--- @return boolean 是否可作为完整目标 argv
function M.executable_name(value)
    return #value <= 160 and value:match("^[A-Za-z0-9_][A-Za-z0-9_.+%-]*$") ~= nil
end

--- 【诊断文本】【去重排序】将集合转换为稳定有界数组
--- @param set table 以文本为键的集合
--- @param limit integer 最大条数
--- @return table 排序后的数组
function M.sorted(set, limit)
    local result = sai.json.array()
    for value in pairs(set) do result[#result + 1] = value end
    table.sort(result)
    while #result > limit do result[#result] = nil end
    return result
end

return M
