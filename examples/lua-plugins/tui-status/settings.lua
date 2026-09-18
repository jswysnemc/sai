local M = {}
local fields = { mode=true, model=true, thinking=true, context=true, cache=true, directory=true, label=true }
local known = { left=true, right=true, separator=true, directory_style=true, label=true, compact_below=true }

--- 【底栏插件】【文本校验】拒绝控制字符并限制 UTF-8 字节数
--- @param value any 设置内容
--- @param name string 设置名称
--- @param max_bytes integer 最大字节数
--- @return string 通过校验的单行文本
local function text(value, name, max_bytes)
    assert(type(value) == "string" and #value <= max_bytes, name .. " must be a short string")
    for _, code in utf8.codes(value) do
        assert(code >= 32 and not (code >= 127 and code <= 159)
            and code ~= 8232 and code ~= 8233, name .. " must contain only single-line text")
    end
    return value
end

--- 【底栏插件】【字段校验】只接受有序、连续且不重复的字段数组
--- @param value any 自定义字段集合
--- @param fallback table 默认字段数组
--- @param name string 左右位置名称
--- @return table 验证后的字段数组
local function order(value, fallback, name)
    if value == nil then return fallback end
    assert(type(value) == "table" and #value <= 7, name .. " must be an array of at most 7 fields")
    local seen = {}
    for key, field in pairs(value) do
        assert(type(key) == "number" and key % 1 == 0 and key >= 1 and key <= #value,
            name .. " must be a continuous array")
        assert(type(field) == "string" and fields[field], "unknown status field in " .. name)
        assert(not seen[field], "duplicate status field: " .. field)
        seen[field] = true
    end
    return value
end

--- 【底栏插件】【设置解析】在配置保存和插件加载时验证全部设置
--- @param config table 插件自身设置
--- @return table 补齐默认值的设置
function M.parse(config)
    for key in pairs(config) do assert(known[key], "unknown setting: " .. tostring(key)) end
    local style = config.directory_style
    if style == nil then style = "name" end
    assert(style == "name" or style == "path", "directory_style must be name or path")
    local compact = config.compact_below
    if compact == nil then compact = 60 end
    assert(type(compact) == "number" and compact % 1 == 0 and compact >= 0 and compact <= 240,
        "compact_below must be an integer from 0 to 240")
    local separator, label = config.separator, config.label
    if separator == nil then separator = " · " end
    if label == nil then label = "Sai" end
    return {
        left = order(config.left, {"mode", "model", "context", "cache"}, "left"),
        right = order(config.right, {"directory"}, "right"),
        separator = text(separator, "separator", 24),
        directory_style = style,
        label = text(label, "label", 80),
        compact_below = compact,
    }
end

return M
