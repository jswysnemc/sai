local M = {}
local defaults = {width_percent=45, height_percent=35, language="en"}

--- 【图片显示】【设置加载】保留终端百分比及显示语言，拒绝未知字段和非法类型
--- @param settings table 插件自己的设置
--- @return table 经过校验的显示配置
function M.load(settings)
    assert(type(settings) == "table", "image-display settings must be an object")
    local config = {}
    for name, value in pairs(defaults) do config[name] = value end
    for name, value in pairs(settings) do
        assert(defaults[name] ~= nil, "unknown image-display setting: " .. tostring(name))
        assert(type(value) == type(defaults[name]), "image-display." .. name .. " has an invalid type")
        config[name] = value
    end
    for _, name in ipairs({"width_percent", "height_percent"}) do
        assert(math.type(config[name]) == "integer" and config[name] >= 0 and config[name] <= 255,
            "image-display." .. name .. " must be an integer between 0 and 255")
    end
    assert(config.language == "en" or config.language == "zh", "image-display.language must be en or zh")
    return config
end

--- 【图片显示】【终端尺寸】显式宽高优先，其次 size，最后使用当前终端百分比
--- @param args table 显示参数
--- @param config table 显示配置
--- @param terminal table|nil 终端行列数量
--- @return string|nil 宽高各自受 300 列和 200 行限制的显示尺寸
function M.size(args, config, terminal)
    local width = math.type(args.width) == "integer" and math.min(math.max(args.width, 0), 300) or 0
    local height = math.type(args.height) == "integer" and math.min(math.max(args.height, 0), 200) or 0
    if width > 0 or height > 0 then
        return (width > 0 and tostring(width) or "") .. "x" .. (height > 0 and tostring(height) or "")
    end
    local size = type(args.size) == "string" and sai.text.trim(args.size) or ""
    if size ~= "" then return size end
    if not terminal then return nil end
    width = math.min(300, math.max(1, math.floor(terminal.columns * config.width_percent / 100)))
    height = math.min(200, math.max(1, math.floor(terminal.rows * config.height_percent / 100)))
    return width .. "x" .. height
end

return M
