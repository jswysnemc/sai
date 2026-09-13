local M = {}
local array_type = getmetatable(sai.json.array())

--- 【会话待办】【数组识别】区分空数组、对象和 null
--- @param value any 待检查的 JSON 值
--- @return boolean 是否为 JSON 数组
function M.is_array(value)
    return type(value) == "table" and getmetatable(value) == array_type
end

--- 【会话待办】【可选文字】只读取非空字符串，按原规则清理 Unicode 空白
--- @param value any 参数值
--- @return string|nil 非空文字或 nil
function M.optional(value)
    if type(value) ~= "string" then return nil end
    local text = sai.text.trim(value)
    return text ~= "" and text or nil
end

--- 【会话待办】【必填文字】沿用单条新增与更新的错误说明
--- @param value any 参数值
--- @param message string 缺失时的错误说明
--- @return string 非空文字
function M.required(value, message)
    local text = M.optional(value)
    assert(text, message)
    return text
end

--- 【会话待办】【批量参数】texts 优先，空数组按原规则回退到 text
--- @param args table 工具参数
--- @return table 非空文字数组
function M.texts(args)
    local result = sai.json.array()
    if M.is_array(args.texts) and #args.texts > 0 then
        for _, text in ipairs(args.texts) do
            result[#result + 1] = M.required(text, "texts must be non-empty strings")
        end
    else
        result[1] = M.required(args.text, "text is required")
    end
    return result
end

--- 【会话待办】【原始序号】保留完整 u64 及非负整数表示，不把浮点数当作原整数
--- @param ctx table 可信输入上下文
--- @return string|nil 原序号的十进制文本
function M.index(ctx)
    local text = ctx.json_integer("/index")
    if not text or text:sub(1, 1) == "-" then return nil end
    return text
end

--- 【会话待办】【序号比较】不经过浮点转换比较大整数与当前数组长度
--- @param text string 十进制整数
--- @param maximum integer 数组最大序号
--- @return boolean 是否超过最大值
function M.exceeds(text, maximum)
    local bound = tostring(maximum)
    return #text > #bound or (#text == #bound and text > bound)
end

--- 【会话待办】【位置解析】id 优先，找不到指定 id 时不能回退到 index
--- @param items table 当前活动清单
--- @param id string|nil 原条目标识
--- @param index string|nil 原整数序号
--- @return integer Lua 使用的 1 起始位置
function M.locate(items, id, index)
    if id then
        for position, item in ipairs(items) do
            if item.id == id then return position end
        end
        error("todo item not found: " .. id)
    end
    assert(index, "todo update/remove requires id or index")
    assert(index ~= "0" and not M.exceeds(index, #items),
        "todo index out of range: " .. index .. " (list has " .. #items .. " items)")
    return tonumber(index)
end

return M
