local M = {}
local result = require("result")
local input_limit = sai.limits.output_bytes

--- 【文本解码】【输入上限】在 Lua 文本转换与宿主调用前限制单个输入
--- @param input string 用户提交的原始文本
--- @return nil 超过字节预算时抛出错误
local function check_input(input)
    if #input > input_limit then error("hash-codec input exceeds plugin size limit", 0) end
end

--- 【文本解码】【字节还原】文本保持 UTF-8 字节，Base64 与 Hex 只去除两端 Unicode 空白
--- @param input string 输入文本
--- @param format string text、base64 或 hex
--- @return string 可以包含 NUL 和非法 UTF-8 的原始字节
function M.bytes(input, format)
    check_input(input)
    if format == "text" then return input end
    if format == "base64" or format == "hex" then
        return sai.encoding.decode(format, sai.text.trim(input))
    end
    error("unsupported input_format: " .. format, 0)
end

--- 【文本解码】【ROT13 字符】只旋转 ASCII 英文字母，保持原大小写
--- @param letter string 单个 ASCII 字母
--- @return string 旋转十三位后的字母
local function rotate(letter)
    local byte = string.byte(letter)
    local base = byte >= 97 and 97 or 65
    return string.char((byte - base + 13) % 26 + base)
end

--- 【文本解码】【格式解析】沿用原版 UTF-8 容错、百分号解码和固定实体替换顺序
--- @param input string 输入文本
--- @param format string base64、hex、url、html 或 rot13
--- @return string 解码后的 UTF-8 文本
local function decode_text(input, format)
    if format == "base64" or format == "hex" then
        return sai.encoding.to_utf8(M.bytes(input, format), true)
    elseif format == "url" then
        return sai.encoding.to_utf8(sai.encoding.decode("url", input))
    elseif format == "html" then
        return (input:gsub("&lt;", "<"):gsub("&gt;", ">"):gsub("&amp;", "&")
            :gsub("&quot;", '"'):gsub("&#39;", "'"))
    elseif format == "rot13" then
        return (input:gsub("[A-Za-z]", rotate))
    end
    error("unsupported input_format: " .. format, 0)
end

--- 【文本解码】【工具结果】验证输入大小并组装保持原有字段的解码结果
--- @param args table 含 input_text 和 input_format；text_encoding 为历史保留字段
--- @return string 含 success 与 decoded_text 的 JSON 文本
function M.run(args)
    check_input(args.input_text)
    return result.decoded(decode_text(args.input_text, args.input_format))
end

return M
