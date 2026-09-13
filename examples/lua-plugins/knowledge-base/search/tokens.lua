local M = {}

--- 【知识库分词】【中英查询】保留 ASCII 单词、中文连续词与相邻二元词，按首次出现去重
--- @param text string 查询文字
--- @return table 原顺序词元数组
function M.query(text)
    local tokens, ascii, chinese = {}, {}, {}
    --- 【知识库分词】【英文片段】提交当前连续 ASCII 单词并清空暂存
    --- @return nil 无参数，词元写入当前结果数组
    local function flush_ascii()
        if #ascii > 0 then tokens[#tokens + 1] = table.concat(ascii); ascii = {} end
    end
    --- 【知识库分词】【中文片段】提交连续中文及相邻二元词并清空暂存
    --- @return nil 无参数，词元写入当前结果数组
    local function flush_chinese()
        if #chinese == 0 then return end
        tokens[#tokens + 1] = table.concat(chinese)
        for index = 1, #chinese - 1 do tokens[#tokens + 1] = chinese[index] .. chinese[index + 1] end
        chinese = {}
    end
    for _, code in utf8.codes(text) do
        if (code >= 48 and code <= 57) or (code >= 65 and code <= 90) or (code >= 97 and code <= 122) then
            ascii[#ascii + 1] = string.char(code >= 65 and code <= 90 and code + 32 or code)
            flush_chinese()
        elseif code >= 0x4e00 and code <= 0x9fff then
            flush_ascii()
            chinese[#chinese + 1] = utf8.char(code)
        else
            flush_ascii(); flush_chinese()
        end
    end
    flush_ascii(); flush_chinese()
    local seen, result = {}, sai.json.array()
    for _, token in ipairs(tokens) do
        if #token > 1 and not seen[token] then result[#result + 1], seen[token] = token, true end
    end
    return result
end

--- 【知识库分词】【字节位置】按原规则最多保留 100 次不重叠命中
--- @param text string 归一后的正文
--- @param needle string 非空词元
--- @return table 零起始字节位置
function M.positions(text, needle)
    local result, start = {}, 1
    while true do
        local position = text:find(needle, start, true)
        if not position then return result end
        result[#result + 1] = position - 1
        if #result >= 100 then return result end
        start = position + #needle
    end
end

return M
