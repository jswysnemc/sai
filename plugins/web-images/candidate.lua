local text = require("strings")
local M = {}

--- 【网页搜图】【像素字段】保留无符号整数并限制到原 u32 范围
--- @param value any 搜索引擎像素字段
--- @return integer 有效像素数量或零
local function dimension(value)
    if type(value) ~= "number" or value < 0 then return 0 end
    if math.type(value) == "integer" or (value >= 2.0 ^ 63 and value <= 2.0 ^ 64) then
        return math.floor(math.min(value, 4294967295))
    end
    return 0
end

--- 【网页搜图】【候选构造】清理引擎字段并生成明确标记为搜索元数据的摘要
--- @param title any 标题
--- @param page any 来源页面
--- @param image any 原图地址
--- @param thumbnail any 缩略图地址
--- @param source string 引擎名称
--- @param width any 原宽度
--- @param height any 原高度
--- @param description any 附加搜索摘要
--- @return table|nil 有效 HTTP(S) 图片候选
function M.build(title, page, image, thumbnail, source, width, height, description)
    image = text.url(text.string(image))
    if not image:match("^https?://") then return nil end
    title, page = text.clean(text.string(title), 180), text.url(text.string(page))
    local parts = {}
    for _, value in ipairs({title, text.clean(text.string(description), 180)}) do
        if value ~= "" then parts[#parts + 1] = value end
    end
    local host = text.host(page)
    if host then parts[#parts + 1] = "来源页面: " .. host end
    return {
        title=title, page_url=page, image_url=image, thumbnail_url=text.url(text.string(thumbnail)),
        source=source, width=dimension(width), height=dimension(height),
        search_description=text.clean(table.concat(parts, "；"), 420),
    }
end

return M
