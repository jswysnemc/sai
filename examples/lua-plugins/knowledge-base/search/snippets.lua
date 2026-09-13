local f32 = require("values").f32
local M = {}

--- 【知识库片段】【字符上下文】以字节命中位置定位，向两侧扩展原规则的字符数
--- @param content string 原始正文
--- @param first integer 零起始命中位置
--- @param last integer 零起始结束位置
--- @param context integer 上下文字符数
--- @return string 合并空白后的原文片段
function M.extract(content, first, last, context)
    local positions = {}
    for position in utf8.codes(content) do positions[#positions + 1] = position - 1 end
    local start, stop, before = 0, #content, 0
    for _, position in ipairs(positions) do if position < first then before = before + 1 else break end end
    if before > context then start = positions[before - context] end
    local after = 0
    for _, position in ipairs(positions) do
        if position >= last then
            if after == context then stop = position; break end
            after = after + 1
        end
    end
    return sai.text.collapse_whitespace(content:sub(start + 1, stop))
end

--- 【知识库片段】【覆盖窗口】用字节距离与词元覆盖率选择首个最优窗口
--- @param positions table 每个词元的命中数组
--- @param tokens table 去重词元
--- @param window integer 原 proximity_window_chars 配置
--- @return table|nil 起点、终点和单精度覆盖率
function M.best(positions, tokens, window)
    local events, best = {}, nil
    for _, token in ipairs(tokens) do
        for _, position in ipairs(positions[token] or {}) do events[#events + 1] = {position=position, token=token, order=#events + 1} end
    end
    table.sort(events, function(a, b) return a.position < b.position or (a.position == b.position and a.order < b.order) end)
    for left, event in ipairs(events) do
        local seen, count, last = {}, 0, event.position
        for right = left, #events do
            local current = events[right]
            if current.position - event.position > window then break end
            if not seen[current.token] then seen[current.token], count = true, count + 1 end
            last = current.position + #current.token
        end
        local coverage = f32(count / math.max(1, #tokens))
        if not best or coverage > best.coverage then best = {start=event.position, ["end"]=last, coverage=coverage} end
    end
    return best
end

--- 【知识库片段】【备用片段】词元优先，最多三段；没有正文命中时保留开头上下文
--- @param content string 原文
--- @param lower string ASCII 归一文字
--- @param tokens table 查询词元
--- @param context integer 上下文长度
--- @return table 原顺序片段数组
function M.fallback(content, lower, tokens, context)
    local result = sai.json.array()
    for _, token in ipairs(tokens) do
        local position = lower:find(token, 1, true)
        if position then result[#result + 1] = M.extract(content, position - 1, position - 1 + #token, context) end
        if #result >= 3 then break end
    end
    if #result == 0 and sai.text.trim(content) ~= "" then
        local offset = utf8.offset(content, context * 2 + 1) or (#content + 1)
        result[1] = sai.text.collapse_whitespace(content:sub(1, offset - 1))
    end
    return result
end

return M
