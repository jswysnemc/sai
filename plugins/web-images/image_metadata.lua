local M = {}

--- 【网页搜图】【媒体类型】保留响应头、签名和最终地址的原判断优先级
--- @param body userdata 下载缓冲
--- @param content_type string 响应媒体类型
--- @param url string 重定向后的地址
--- @return string|nil 支持的图片媒体类型
function M.mime(body, content_type, url)
    local header = sai.text.trim((content_type:match("^[^;]*"))):lower()
    local allowed = {["image/jpeg"]=true, ["image/jpg"]=true, ["image/png"]=true,
        ["image/gif"]=true, ["image/webp"]=true, ["image/bmp"]=true}
    if allowed[header] then return header end
    local prefix = body:bytes(0, 12)
    if prefix:sub(1, 3) == "\255\216\255" then return "image/jpeg" end
    if prefix:sub(1, 8) == "\137PNG\r\n\26\n" then return "image/png" end
    if prefix:sub(1, 6) == "GIF87a" or prefix:sub(1, 6) == "GIF89a" then return "image/gif" end
    if prefix:sub(1, 4) == "RIFF" and prefix:sub(9, 12) == "WEBP" then return "image/webp" end
    if prefix:sub(1, 2) == "BM" then return "image/bmp" end
    local suffix = url:lower():match("%.([^.]+)$")
    return ({jpg="image/jpeg", jpeg="image/jpeg", png="image/png", gif="image/gif", webp="image/webp", bmp="image/bmp"})[suffix]
end

--- 【网页搜图】【整数解码】从有界字节片段读取无符号整数
--- @param value string 原始字节
--- @param first integer 首个字节的一基位置
--- @param count integer 字节数量
--- @param little boolean 是否为小端序
--- @return integer 解码后的数值
local function integer(value, first, count, little)
    local result = 0
    for index = 0, count - 1 do
        local offset = little and (first + count - index - 1) or (first + index)
        result = result * 256 + value:byte(offset)
    end
    return result
end

--- 【网页搜图】【JPEG 尺寸】分块扫描标记，不把整张图片复制进 Lua 堆
--- @param body userdata 下载缓冲
--- @return integer, integer 宽度和高度，无法识别时均为零
local function jpeg_dimensions(body)
    local size, cached, start = body:len(), "", 0
    --- 【网页搜图】【JPEG 字节】缓存相邻标记所在的数据块
    --- @param index integer 一基字节位置
    --- @return integer 当前字节
    local function byte(index)
        if index < start or index >= start + #cached then
            start, cached = index, body:bytes(index - 1, 16384)
        end
        return cached:byte(index - start + 1)
    end
    local sof = {[192]=true, [193]=true, [194]=true, [195]=true, [197]=true, [198]=true, [199]=true,
        [201]=true, [202]=true, [203]=true, [205]=true, [206]=true, [207]=true}
    local index = 3
    while index + 8 < size do
        if byte(index) ~= 255 then index = index + 1 else
            while index <= size and byte(index) == 255 do index = index + 1 end
            if index > size then break end
            local marker = byte(index)
            index = index + 1
            if marker ~= 216 and marker ~= 217 and marker ~= 1 and not (marker >= 208 and marker <= 215) then
                if marker == 218 or index + 1 > size then break end
                local length = byte(index) * 256 + byte(index + 1)
                if length < 2 or index + length - 1 > size then break end
                if sof[marker] and index + 6 <= size then
                    return byte(index + 5) * 256 + byte(index + 6), byte(index + 3) * 256 + byte(index + 4)
                end
                index = index + length
            end
        end
    end
    return 0, 0
end

--- 【网页搜图】【图片尺寸】解析原五类格式的头部，格式与媒体类型不符时保留未知尺寸
--- @param body userdata 原始下载缓冲
--- @param mime string 已选择的媒体类型
--- @return integer, integer 图片宽度和高度
function M.dimensions(body, mime)
    local value = body:bytes(0, 32)
    if mime == "image/png" and #value >= 24 and value:sub(1, 8) == "\137PNG\r\n\26\n" then
        return integer(value, 17, 4, false), integer(value, 21, 4, false)
    elseif mime == "image/gif" and #value >= 10 and (value:sub(1, 6) == "GIF87a" or value:sub(1, 6) == "GIF89a") then
        return integer(value, 7, 2, true), integer(value, 9, 2, true)
    elseif mime == "image/bmp" and #value >= 26 and value:sub(1, 2) == "BM" then
        local width, height = integer(value, 19, 4, true), integer(value, 23, 4, true)
        return math.abs(width >= 2147483648 and width - 4294967296 or width), math.abs(height >= 2147483648 and height - 4294967296 or height)
    elseif mime == "image/webp" and #value >= 30 and value:sub(1, 4) == "RIFF" and value:sub(9, 12) == "WEBP" then
        local kind = value:sub(13, 16)
        if kind == "VP8X" then return 1 + integer(value, 25, 3, true), 1 + integer(value, 28, 3, true)
        elseif kind == "VP8 " then return integer(value, 27, 2, true) & 0x3fff, integer(value, 29, 2, true) & 0x3fff
        elseif kind == "VP8L" then
            return 1 + (((value:byte(23) & 0x3f) << 8) | value:byte(22)),
                1 + (((value:byte(25) & 0x0f) << 10) | (value:byte(24) << 2) | ((value:byte(23) & 0xc0) >> 6))
        end
    elseif (mime == "image/jpeg" or mime == "image/jpg") and value:sub(1, 2) == "\255\216" then
        return jpeg_dimensions(body)
    end
    return 0, 0
end

--- 【网页搜图】【文件扩展名】按已识别媒体类型选择缓存扩展名
--- @param mime string 图片媒体类型
--- @return string 带点的扩展名
function M.extension(mime)
    return ({["image/png"]=".png", ["image/gif"]=".gif", ["image/webp"]=".webp", ["image/bmp"]=".bmp"})[mime] or ".jpg"
end

return M
