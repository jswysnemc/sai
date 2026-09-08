local M = {}

--- 【游戏信号】【字符裁剪】按 Unicode 字符边界截取文本，不添加省略标记
--- @param value string 原文本
--- @param limit integer 最多保留的字符数
--- @return string 截取后的 UTF-8 文本
function M.clip(value, limit)
    local offset = utf8.offset(value, limit + 1)
    return offset and value:sub(1, offset - 1) or value
end

--- 【游戏信号】【大小写】仅转换 ASCII 大写字母，保持其他 Unicode 字符不变
--- @param value string 原文本
--- @return string ASCII 小写文本
function M.ascii_lower(value)
    return (value:gsub("[A-Z]", string.lower))
end

--- 【游戏信号】【正文摘录】合并 Unicode 空白并按原规则截取正文
--- @param value string 页面正文
--- @param limit integer 最多保留的字符数
--- @return string 单行摘录
function M.excerpt(value, limit)
    return M.clip(sai.text.collapse_whitespace(value), limit)
end

--- 【游戏信号】【标签取值】读取首个完整标签之后的非空行
--- @param value string 页面正文
--- @param label string 精确标签
--- @return string|nil 最多 120 字符的值，没有后续行时返回 nil
function M.after_label(value, label)
    local matched = false
    for line in (value .. "\n"):gmatch("(.-)\n") do
        line = sai.text.trim(line)
        if line ~= "" then
            if matched then return M.clip(line, 120) end
            matched = line == label
        end
    end
end

--- 【游戏信号】【章节摘录】保留原 split 规则，只读取首个起始标签后的片段
--- @param value string 页面正文
--- @param first string 起始标签
--- @param last string 终止标签
--- @param limit integer 字符上限
--- @return string|nil 章节摘录，没有起始标签时返回 nil
function M.section(value, first, last, limit)
    local _, start = value:find(first, 1, true)
    if not start then return nil end
    local section = value:sub(start + 1)
    local repeated = section:find(first, 1, true)
    if repeated then section = section:sub(1, repeated - 1) end
    local ending = section:find(last, 1, true)
    if ending then section = section:sub(1, ending - 1) end
    return M.excerpt(section, limit)
end

return M
