local values = require("values")
local M = {}
local defaults = {
    libraries={}, width_percent=35, height_percent=25, max_image_mb=10,
    allow_gif_animation=false, auto_send_enabled=false,
    auto_send_probability=0.2, auto_send_min_confidence=0.8, language="en",
    builtin_dirs=sai.json.array({"src/memes", "/usr/share/sai/memes"}),
    user_dir=".sai/memes", state_dir=".sai/state/memes", input_paths=sai.json.array({"."}),
}

--- 【表情库】【配置校验】只接收本插件设置，拒绝无界值及未知字段
--- @param source table 显式设置与旧配置的定向合并结果
--- @return table 独立且经过校验的配置
function M.load(source)
    assert(type(source) == "table", "memes settings must be an object")
    local config = values.copy(defaults)
    for key, value in pairs(source) do
        assert(defaults[key] ~= nil, "unknown memes setting: " .. tostring(key))
        assert(type(value) == type(defaults[key]), "memes." .. key .. " has an invalid type")
        config[key] = value
    end
    for _, key in ipairs({"width_percent", "height_percent"}) do
        assert(math.type(config[key]) == "integer" and config[key] >= 0 and config[key] <= 255,
            "memes." .. key .. " must be an integer between 0 and 255")
    end
    assert(config.max_image_mb >= 0 and (math.type(config.max_image_mb) == "integer"
        or (config.max_image_mb >= 9223372036854775808 and config.max_image_mb <= 18446744073709551615.0)),
        "memes.max_image_mb must be a nonnegative integer")
    for _, key in ipairs({"auto_send_probability", "auto_send_min_confidence"}) do
        local value = config[key]
        assert(value == value and value > -math.huge and value < math.huge, "memes." .. key .. " must be finite")
        config[key] = values.float32(value)
        assert(math.abs(config[key]) < math.huge, "memes." .. key .. " exceeds f32 range")
    end
    assert(not values.is_array(config.libraries), "memes.libraries must be an object")
    for key, value in pairs(config.libraries) do
        assert(type(key) == "string" and type(value) == "string", "memes.libraries must contain strings")
    end
    for _, key in ipairs({"builtin_dirs", "input_paths"}) do
        assert(values.is_array(config[key]) and #config[key] <= 32, "memes." .. key .. " must be a bounded array")
        for _, path in ipairs(config[key]) do
            assert(type(path) == "string" and path ~= "" and #path <= 4096, "invalid memes directory")
        end
    end
    assert(#config.builtin_dirs > 0, "memes.builtin_dirs must not be empty")
    for _, key in ipairs({"user_dir", "state_dir"}) do
        assert(config[key] ~= "" and #config[key] <= 4096, "invalid memes." .. key)
    end
    assert(config.language == "en" or config.language == "zh", "memes.language must be en or zh")
    return config
end

--- 【表情库】【显示尺寸】原始非负宽高优先，其次 size，最后使用终端百分比
--- @param args table 显示参数
--- @param config table 插件配置
--- @param terminal table|nil 终端列数和行数
--- @param ctx table 提供原始整数查询的可信上下文
--- @return string|nil 160 列和 80 行以内的尺寸
function M.size(args, config, terminal, ctx)
    local width = values.integer(ctx, "width", 0, 160)
    local height = values.integer(ctx, "height", 0, 80)
    if width > 0 or height > 0 then
        return (width > 0 and tostring(width) or "") .. "x" .. (height > 0 and tostring(height) or "")
    end
    local size = sai.text.trim(values.string(args.size))
    if size ~= "" then return size end
    return M.configured(config, terminal)
end

--- 【表情库】【默认尺寸】自动发送和手工显示共用终端百分比
--- @param config table 显示设置
--- @param terminal table|nil 终端列数和行数
--- @return string|nil 有界默认尺寸
function M.configured(config, terminal)
    if not terminal then return nil end
    local width = math.min(160, math.max(1, math.floor(terminal.columns * config.width_percent / 100)))
    local height = math.min(80, math.max(1, math.floor(terminal.rows * config.height_percent / 100)))
    return width .. "x" .. height
end

return M
