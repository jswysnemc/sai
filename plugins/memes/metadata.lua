local values = require("values")
local M = {}

--- 【表情库】【字段读取】按原版规则读取元数据中的字符串并清除两侧空白
--- @param object any 元数据对象
--- @param key string 字段名
--- @return string 有效字符串或空字符串
local function field(object, key)
    return sai.text.trim(values.string(type(object) == "table" and object[key] or nil))
end

--- 【表情库】【手工分支】任一元数据字段存在就使用手工模式，包括显式 null
--- @param args table 原始工具参数
--- @return boolean 是否选择手工元数据
function M.supplied(args)
    for _, key in ipairs({"name_zh", "name_en", "description", "usage", "avoid", "tags"}) do
        if args[key] ~= nil then return true end
    end
    return false
end

--- 【表情库】【手工元数据】保留必填字段规则及标签过滤，不验证图片实际格式
--- @param args table 用户字段
--- @param id string 完整内容标识
--- @param file string 相对图片路径
--- @param mime string 扩展名对应类型
--- @param animated boolean 是否为 GIF
--- @return table 完整条目
function M.manual(args, id, file, mime, animated)
    local item = {
        id=id, file=file, mime_type=mime, animated=animated,
        name={zh=field(args, "name_zh"), en=field(args, "name_en")},
        description=field(args, "description"), usage=field(args, "usage"),
        avoid=field(args, "avoid"), tags=values.strings(args.tags),
    }
    assert(item.name.zh ~= "" and item.description ~= "" and item.usage ~= "",
        "name_zh, description, and usage are required when supplying metadata manually")
    return item
end

--- 【表情库】【视觉元数据】从模型对象读取旧字段，save 不改变原版选择规则
--- @param data any 模型 JSON
--- @param id string 完整内容标识
--- @param file string 相对图片路径
--- @param mime string 图片类型
--- @param animated boolean 动画标记
--- @return table 字段完整的条目
function M.vision(data, id, file, mime, animated)
    data = type(data) == "table" and data or {}
    local item = {
        id=id, file=file, mime_type=mime, animated=animated,
        name={zh=field(data.name, "zh"), en=field(data.name, "en")},
        description=field(data, "description"), usage=field(data, "usage"),
        avoid=field(data, "avoid"), tags=values.strings(data.tags),
    }
    assert(item.name.zh ~= "" and item.description ~= "" and item.usage ~= "", "vision metadata is incomplete")
    return item
end

--- 【表情库】【局部更新】空名称和用途不覆盖，avoid 可清空，tags 出现时重新过滤
--- @param item table 独立条目副本
--- @param args table 更新字段
--- @return table 更新后的同一条目
function M.update(item, args)
    for _, key in ipairs({"zh", "en"}) do
        local value = field(args, "name_" .. key)
        if value ~= "" then item.name[key] = value end
    end
    for _, key in ipairs({"description", "usage"}) do
        local value = field(args, key)
        if value ~= "" then item[key] = value end
    end
    if type(args.avoid) == "string" then item.avoid = field(args, "avoid") end
    if args.tags ~= nil then item.tags = values.strings(args.tags) end
    return item
end

return M
