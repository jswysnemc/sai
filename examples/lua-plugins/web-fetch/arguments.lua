local M = {}

--- 【网页读取】【整数收窄】从原始 JSON 整数保留完整无符号范围及原默认值
--- @param value string|nil 原整数十进制文本
--- @param fallback integer 缺失、负数和浮点输入的默认值
--- @param minimum integer 最小结果
--- @param maximum integer 最大结果
--- @return integer 收窄后的数量
local function unsigned(value, fallback, minimum, maximum)
    if value == nil or value:sub(1, 1) == "-" then
        return fallback
    end
    return math.max(minimum, math.min(maximum, tonumber(value)))
end

--- 【网页读取】【参数归一】保留 Unicode 空白清理、格式缺省和整数边界
--- @param args table 工具输入
--- @param ctx table 提供原始 JSON 整数的调用上下文
--- @return table 请求地址、输出格式、毫秒期限与字符上限
function M.parse(args, ctx)
    local url = sai.text.trim(type(args.url) == "string" and args.url or "")
    if url:sub(1, 7) ~= "http://" and url:sub(1, 8) ~= "https://" then
        error("URL must start with http:// or https://")
    end
    return {
        url = url,
        format = type(args.format) == "string" and args.format or "markdown",
        timeout_ms = unsigned(ctx.json_integer("/timeout"), 30, 0, 120) * 1000,
        max_chars = unsigned(ctx.json_integer("/max_chars"), 24000, 1, 80000),
    }
end

return M
