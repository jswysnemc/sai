local M = {}

--- 【知识库数值】【单精度兼容】每一步按原 f32 精度舍入
--- @param value number 中间计算值
--- @return number 与原单精度相同的数值
function M.f32(value) return (string.unpack("f", string.pack("f", value))) end

--- 【知识库数值】【一位小数】保留原评分输出中的单精度舍入
--- @param value number 原评分
--- @return number 舍入后的单精度值
function M.score(value)
    local scaled = M.f32(value * 10)
    return M.f32((scaled < 0 and math.ceil(scaled - 0.5) or math.floor(scaled + 0.5)) / 10)
end

--- 【知识库参数】【原始无符号整数】工具使用原 JSON；命令传入十进制文字
--- @param args table 参数
--- @param ctx table 可信上下文
--- @param key string 字段
--- @param default string|nil 缺失或错误表示的默认值
--- @return string|nil 完整非负整数
function M.integer(args, ctx, key, default)
    local value
    if ctx.json_integer then value = ctx.json_integer("/" .. key)
    elseif type(args[key]) == "string" and args[key]:match("^%d+$") then value = args[key]
    elseif math.type(args[key]) == "integer" then value = tostring(args[key]) end
    if not value or value:sub(1, 1) == "-" then return default end
    return value
end

--- 【知识库参数】【完整整数比较】不把 u64 最大值转换为浮点
--- @param decimal string 十进制整数
--- @param limit integer 有界比较值
--- @return boolean 是否超过上限
function M.exceeds(decimal, limit)
    local text = tostring(limit)
    return #decimal > #text or (#decimal == #text and decimal > text)
end

--- 【知识库参数】【分页限制】按原有范围收窄完整整数
--- @param decimal string|nil 可选十进制数
--- @param default integer 默认值
--- @param maximum integer 上限
--- @return integer 1 至上限之间的值
function M.limit(decimal, default, maximum)
    if not decimal then return math.max(1, math.min(maximum, default)) end
    if M.exceeds(decimal, maximum) then return maximum end
    return math.max(1, tonumber(decimal))
end

--- 【知识库文字】【ASCII 大小写】只转换 ASCII，与旧检索和扩展名规则一致
--- @param text string 原文
--- @return string 大小写归一后的文字
function M.lower(text) return (text:gsub("[A-Z]", function(ch) return string.char(ch:byte() + 32) end)) end

--- 【知识库文字】【有界 UTF-8】按字节上限截断，但不切断末尾的多字节字符
--- @param text string 有效 UTF-8 文字
--- @param maximum integer 最大字节数
--- @return string 完整字符组成的前缀
function M.truncate(text, maximum)
    local last = math.min(#text, maximum)
    while last > 0 do
        local next_byte = text:byte(last + 1)
        if not next_byte or next_byte < 128 or next_byte >= 192 then break end
        last = last - 1
    end
    return text:sub(1, last)
end

--- 【知识库参数】【必填文字】保留原 Unicode 去空白和错误说明
--- @param args table 参数
--- @param key string 必填字段
--- @return string 非空文字
function M.required(args, key)
    local value = type(args[key]) == "string" and sai.text.trim(args[key]) or ""
    assert(value ~= "", key .. " is required")
    return value
end

--- 【知识库行处理】【原始行】匹配 Rust lines，末尾换行不产生额外空行
--- @param text string UTF-8 正文
--- @return table 保留空行的字符串数组
function M.lines(text)
    local lines, start = {}, 1
    while true do
        local stop = text:find("\n", start, true)
        if not stop then
            if start <= #text then lines[#lines + 1] = text:sub(start) end
            return lines
        end
        local line = text:sub(start, stop - 1)
        if line:sub(-1) == "\r" then line = line:sub(1, -2) end
        lines[#lines + 1], start = line, stop + 1
    end
end

--- 【知识库排序】【稳定顺序】同分条目保持原输入顺序
--- @param items table 结果数组
--- @return table 原地按分数倒序排列的数组
function M.sort(items)
    local positions = {}
    for index, item in ipairs(items) do positions[item] = index end
    table.sort(items, function(a, b)
        if a.score == b.score then return positions[a] < positions[b] end
        return a.score > b.score
    end)
    return items
end

return M
