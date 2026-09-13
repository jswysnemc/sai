local render = require("render")
local M = {}

--- 【骰子工具】【无符号参数】沿用原整数默认值和收窄规则，完整接收 JSON 的无符号 64 位范围
--- @param integer string|nil 原 JSON 整数的精确十进制文本
--- @param fallback integer 缺失、负数或浮点输入的默认值
--- @param minimum integer 最小返回值
--- @param maximum integer 最大返回值
--- @return integer 收窄后的有效数量
local function bounded_unsigned(integer, fallback, minimum, maximum)
    if integer == nil or integer:sub(1, 1) == "-" then
        return fallback
    end
    return math.max(minimum, math.min(maximum, tonumber(integer)))
end

--- 【骰子工具】【随机投掷】按有效数量和面数抽取点数，精确计算有符号总和
--- @param args table 可选 count、sides 和 modifier 参数
--- @param ctx table 提供原始 JSON 整数查询的本次上下文
--- @return string 保留原字段和紧凑排版的 JSON，修正总和溢出时返回错误
function M.roll(args, ctx)
    -- 1. 【骰子工具】【数值兼容】原始整数查询区分超大整数、浮点表示和缺失值
    local count = bounded_unsigned(ctx.json_integer("/count"), 1, 1, 100)
    local sides = bounded_unsigned(ctx.json_integer("/sides"), 6, 2, 1000)
    local modifier = math.type(args.modifier) == "integer" and args.modifier or 0
    local rolls = sai.json.array()
    local total = 0

    -- 2. 【骰子工具】【独立抽取】最多抽取一百次，每颗骰子均覆盖完整闭区间
    for index = 1, count do
        local value = math.random(1, sides)
        rolls[index] = value
        total = total + value
    end

    -- 3. 【骰子工具】【溢出检查】总点数始终为正，先检查上界再执行整数加法
    if modifier > math.maxinteger - total then
        error("modified dice total exceeds signed 64-bit range")
    end
    return render.dice({
        count = count,
        sides = sides,
        rolls = rolls,
        total = total,
        modifier = modifier,
        modified_total = total + modifier,
    })
end

return M
