local values = require("values")
local paths = require("paths")
local M = {}

--- 【表情索引】【对象检查】拒绝数组和 null 冒充记录对象
--- @param value any JSON 值
--- @return table 已确认对象
local function object(value)
    assert(type(value) == "table" and not values.is_array(value), "meme index requires JSON objects")
    return value
end

--- 【表情索引】【字符串列表】校验旧索引数组，不静默丢弃损坏字段
--- @param items any 原数组
--- @return table 通过类型检查的数组
local function strings(items)
    assert(values.is_array(items) and #items <= 4096, "invalid meme index string array")
    for _, item in ipairs(items) do assert(type(item) == "string", "invalid meme index string") end
    return items
end

--- 【表情索引】【条目结构】保留原默认字段，同时拒绝危险图片路径和不完整元数据
--- @param item any 待读取或保存的条目
--- @return table 校验后的同一条目
function M.item(item)
    object(item)
    for _, key in ipairs({"id", "file", "mime_type", "description", "usage", "avoid"}) do
        assert(type(item[key]) == "string", "invalid meme index field: " .. key)
    end
    assert(values.id_part(item.id) ~= "" and #item.id <= 256, "invalid meme index id")
    paths.relative(item.file)
    assert(item.file:sub(1, 7) == "images/", "meme image must be inside images/")
    object(item.name)
    for _, key in ipairs({"zh", "en"}) do
        if item.name[key] == nil then item.name[key] = "" end
        assert(type(item.name[key]) == "string", "invalid meme name")
    end
    if item.animated == nil then item.animated = false end
    assert(type(item.animated) == "boolean", "invalid meme animated flag")
    if item.tags == nil then item.tags = sai.json.array() end
    strings(item.tags)
    return item
end

--- 【表情索引】【完整校验】限制记录规模，校验待删除记录与条目的固定关联
--- @param index any JSON 索引
--- @return table 可供业务使用的索引
function M.validate(index)
    object(index)
    if index.library == nil then index.library = "" end
    if index.version == nil then index.version = 0 end
    if index.memes == nil then index.memes = sai.json.array() end
    if index.disabled_ids == nil then index.disabled_ids = sai.json.array() end
    if index.pending_deletions == nil then index.pending_deletions = sai.json.array() end
    assert(type(index.library) == "string", "invalid meme index library")
    assert(math.type(index.version) == "integer" and index.version >= 0 and index.version <= 4294967295,
        "invalid meme index version")
    assert(values.is_array(index.memes) and #index.memes <= 4096, "invalid meme index entries")
    strings(index.disabled_ids)
    local identifiers, files = {}, {}
    for _, item in ipairs(index.memes) do
        M.item(item)
        local id = values.id_part(item.id)
        assert(not identifiers[id], "duplicate meme index id")
        assert(not files[item.file], "duplicate meme image path")
        identifiers[id], files[item.file] = item, true
    end
    assert(values.is_array(index.pending_deletions) and #index.pending_deletions <= 64,
        "invalid pending meme deletions")
    local pending_ids = {}
    for _, pending in ipairs(index.pending_deletions) do
        object(pending)
        assert(type(pending.id) == "string" and type(pending.file) == "string", "invalid pending meme target")
        assert(type(pending.token) == "string" and pending.token:match("^%x+$") and #pending.token == 32,
            "invalid pending meme token")
        assert(pending.mode == "remove" or pending.mode == "trash", "invalid pending meme removal mode")
        local id = values.id_part(pending.id)
        local item = identifiers[id]
        assert(item and item.file == pending.file and not pending_ids[id], "pending meme target changed")
        pending_ids[id] = true
    end
    return index
end

return M
