local M = {}

--- 【底栏插件】【数量缩写】将上下文窗口大小格式化为紧凑单位
--- @param value number token 数量
--- @return string 数量文本
local function tokens(value)
    if value >= 1000000 then return string.format("%.1fm", value / 1000000) end
    if value >= 1000 then return string.format("%.0fk", value / 1000) end
    return tostring(value)
end

--- 【底栏插件】【字段格式】组织左右字段，缺少缓存读数时不显示空占位
--- @param event table 宿主传入的底栏状态
--- @param config table 已校验配置
--- @return table 左右纯文本
function M.render(event, config)
    local directory = event.directory
    if config.directory_style == "name" then
        directory = directory:gsub("[/\\]+$", ""):match("([^/\\]+)$") or event.directory
    end
    local cache = ""
    if type(event.cache_hit_ratio) == "number" then
        cache = string.format("cache %.0f%%", math.max(0, math.min(1, event.cache_hit_ratio)) * 100)
    end
    local values = {
        mode = event.mode,
        model = event.model,
        thinking = event.thinking,
        context = string.format("%.1f%%/%s", math.max(0, event.context_ratio) * 100, tokens(event.context_window_tokens)),
        cache = cache,
        directory = directory,
        label = config.label,
    }
    local compact = event.columns < config.compact_below
    --- 【底栏插件】【段落组合】窄终端隐藏次要字段，保留配置顺序
    --- @param order table 字段顺序
    --- @return string 单侧展示文本
    local function side(order)
        local parts = {}
        for _, field in ipairs(order) do
            local value = values[field]
            if value ~= "" and not (compact and (field == "thinking" or field == "cache")) then
                parts[#parts + 1] = value
            end
        end
        return table.concat(parts, config.separator)
    end
    return { left = side(config.left), right = side(config.right) }
end

return M
