local M = {}

--- 【知识库嵌入】【字符分块】按字符分块和重叠，索引起止继续保存零起始字节位置
--- @param content string UTF-8 正文
--- @param size integer 每块字符数
--- @param overlap integer 重叠字符数
--- @return table 非空文本块，编号从零开始
function M.build(content, size, overlap)
    local offsets, chunks = {}, sai.json.array()
    for position in utf8.codes(content) do offsets[#offsets + 1] = position end
    local total, first = #offsets, 1
    offsets[total + 1] = #content + 1
    while first <= total do
        local last = math.min(first + size, total + 1)
        local text = content:sub(offsets[first], offsets[last] - 1)
        if sai.text.trim(text) ~= "" then
            chunks[#chunks + 1] = {index=#chunks, start=offsets[first] - 1, ["end"]=offsets[last] - 1, text=text}
        end
        if last == total + 1 then break end
        first = math.max(last - overlap, first + 1)
    end
    return chunks
end

return M
