local M = {}

--- 【骰子工具】【结果格式】保留原 JSON 字段顺序与整数精度，避免 Lua 表遍历改变文本
--- @param result table 已校验的数量、面数、点数、总和与修正值
--- @return string 原顺序的紧凑 JSON 文本
function M.dice(result)
    return string.format(
        '{"ok":true,"count":%d,"sides":%d,"rolls":%s,"total":%d,"modifier":%d,"modified_total":%d}',
        result.count,
        result.sides,
        sai.json.encode(result.rolls),
        result.total,
        result.modifier,
        result.modified_total
    )
end

return M
