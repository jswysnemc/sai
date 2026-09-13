local parser = {}
local targets = { "initialPageConfig", "active_changes", "initialCalendarData", "component_uptimes" }
local marker = 'self.__next_f.push([1,"'

--- 【服务状态】【字段提取】从嵌套 JSON 中取得状态页面需要的字段
--- @param value any 解码后的节点
--- @param collected table 已找到的字段
--- @param depth number 递归深度
local function collect(value, collected, depth)
    assert(depth <= 128, "status page data nesting exceeds limit")
    if type(value) ~= "table" then return end
    for _, key in ipairs(targets) do
        if collected[key] == nil and value[key] ~= nil then collected[key] = value[key] end
    end
    for _, child in pairs(value) do collect(child, collected, depth + 1) end
end

--- 【服务状态】【载荷提取】跳过字符串转义，定位 Next.js 数据片段边界
--- @param html string 页面源码
--- @param first number JSON 字符串正文起点
--- @return string|nil 尚未解码的字符串正文
local function payload(html, first)
    local position = first
    while position <= #html do
        local byte = html:sub(position, position)
        if byte == "\\" then
            position = position + 2
        elseif byte == '"' and html:sub(position + 1, position + 2) == "])" then
            return html:sub(first, position - 1)
        else
            position = position + 1
        end
    end
    return nil
end

--- 【服务状态】【页面解析】读取页面中的结构化状态数据
--- @param html string 官方状态页面源码
--- @return table 状态页组件、可用率和事件数据
function parser.parse(html)
    local collected, offset = {}, 1
    while true do
        local _, last = html:find(marker, offset, true)
        if not last then break end
        local raw = payload(html, last + 1)
        if raw then
            local ok, decoded = pcall(sai.json.decode, '"' .. raw .. '"')
            if ok and type(decoded) == "string" then
                local colon = decoded:find(":", 1, true)
                if colon then
                    local parsed, value = pcall(sai.json.decode, decoded:sub(colon + 1))
                    if parsed then collect(value, collected, 0) end
                end
            end
        end
        offset = last + 1
    end
    assert(next(collected) ~= nil, "failed to parse DeepSeek status page data")
    return collected
end

return parser
