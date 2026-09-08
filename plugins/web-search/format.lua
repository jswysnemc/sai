local M = {}

--- 【网页搜索】【字段读取】仅从 JSON 对象读取字段，保留显式 null 和 false
--- @param value any 响应值
--- @param key string 字段名称
--- @return any 字段值或 nil
local function field(value, key)
    if type(value) == "table" then return value[key] end
end

--- 【网页搜索】【字段回退】只有字段缺失时才使用备用值
--- @param value any 首选字段
--- @param fallback any 备用字段
--- @return any 首个存在的字段值
local function present(value, fallback)
    if value ~= nil then return value end
    return fallback
end

--- 【网页搜索】【文本字段】与原始 JSON 字符串类型判断保持一致
--- @param value any 字段值
--- @param fallback string 非字符串时的默认值
--- @return string 有效文本
local function text(value, fallback)
    if type(value) == "string" then return value end
    return fallback
end

--- 【网页搜索】【文本裁剪】按 Unicode 字符裁剪，刚好达到上限时保留原文
--- @param value string 原文
--- @param maximum integer 最大字符数
--- @return string 原文或带三个点省略标记的文本
local function clip(value, maximum)
    local stop = utf8.offset(value, maximum + 1)
    if stop and stop <= #value then return value:sub(1, stop - 1) .. "..." end
    return value
end

--- 【网页搜索】【结果格式】统一普通搜索结果及 Firecrawl 元数据的 Markdown 输出
--- @param query string 查询词
--- @param provider string 供应商显示名称
--- @param results table 已确认的 JSON 结果数组
--- @return string 与旧版字段优先级和裁剪规则一致的 Markdown
function M.render(query, provider, results)
    local lines = { "## Search results for: " .. query, "**Provider**: " .. provider .. "\n" }
    for index, item in ipairs(results) do
        local metadata = field(item, "metadata")
        local title = text(present(field(item, "title"), field(metadata, "title")), "Untitled")
        local url = text(present(field(item, "url"), present(field(metadata, "sourceURL"), field(metadata, "url"))), "")
        local snippet = text(present(field(item, "content"), present(field(item, "snippet"), field(item, "description"))), "")
        local raw = text(present(field(item, "raw_content"), field(item, "markdown")), "")
        lines[#lines + 1] = "### " .. index .. ". " .. title
        if url ~= "" then lines[#lines + 1] = "**URL**: " .. url end
        if snippet ~= "" then lines[#lines + 1] = "**Snippet**: " .. clip(snippet, 500) end
        if raw ~= "" then lines[#lines + 1] = "**Content**: " .. clip(raw, 800) end
        lines[#lines + 1] = ""
    end
    return table.concat(lines, "\n")
end

return M
