local M = {}
local array_metatable = getmetatable(sai.json.array())

--- 【网页搜索】【凭据使用】从宿主准备的配置中选择首个非空密钥
--- @param config table 搜索配置
--- @param provider string 供应商配置前缀
--- @return string 已解析密钥，缺失时抛出不包含密钥内容的错误
function M.key(config, provider)
    for _, value in ipairs(config[provider .. "_api_keys"]) do
        local key = sai.text.trim(value)
        if key ~= "" then
            assert(key:sub(1, 5) ~= "$env:", "API key environment reference was not resolved by the host")
            return key
        end
    end
    error("missing " .. provider .. " API key")
end

--- 【网页搜索】【请求执行】统一响应限制和配置超时，HTTP 错误交给供应商回退
--- @param url string 完整请求地址
--- @param method string HTTP 方法
--- @param headers table 请求头
--- @param body string|nil 可选请求正文
--- @param config table 包含 timeout_seconds 的设置
--- @return string 已解码响应正文
function M.request(url, method, headers, body, config)
    local response = sai.http.request({
        url = url, method = method, headers = headers, body = body,
        max_bytes = 4194304, timeout_ms = config.timeout_seconds * 1000,
    })
    assert(response.status < 400, "search provider returned HTTP " .. response.status)
    return response.text
end

--- 【网页搜索】【JSON POST】使用已有 Bearer 认证方式调用查询端点
--- @param provider string 供应商配置前缀
--- @param payload table 搜索请求数据
--- @param config table 搜索配置
--- @return any 已解析的 JSON 响应
function M.post(provider, payload, config)
    local key = M.key(config, provider)
    return sai.json.decode(M.request(sai.text.trim(config[provider .. "_base_url"]), "POST", {
        authorization = "Bearer " .. key, ["content-type"] = "application/json",
    }, sai.json.encode(payload), config))
end

--- 【网页搜索】【数组读取】只接受 JSON 数组，其他类型按原版空数组规则处理
--- @param data any JSON 响应
--- @param name string 数组字段名称
--- @param maximum integer|nil 可选结果数量限制
--- @return table 响应数组或限制后的数组
function M.results(data, name, maximum)
    local items = type(data) == "table" and data[name] or nil
    if type(items) ~= "table" or getmetatable(items) ~= array_metatable then return {} end
    if maximum == nil then return items end
    local result = {}
    for index = 1, math.min(#items, maximum) do result[index] = items[index] end
    return result
end

--- 【网页搜索】【查询参数】按表单编码追加参数，保留原查询和片段的位置
--- @param url string 供应商地址
--- @param pairs_list table 参数名称和值组成的有序二元组
--- @return string 追加查询参数后的地址
function M.append_query(url, pairs_list)
    local fragment = url:find("#", 1, true)
    local suffix = fragment and url:sub(fragment) or ""
    local base = fragment and url:sub(1, fragment - 1) or url
    local values = {}
    for _, pair in ipairs(pairs_list) do
        local name = sai.text.url_encode(pair[1]):gsub("%%20", "+"):gsub("~", "%%7E"):gsub("%%2A", "*")
        local value = sai.text.url_encode(pair[2]):gsub("%%20", "+"):gsub("~", "%%7E"):gsub("%%2A", "*")
        values[#values + 1] = name .. "=" .. value
    end
    local separator = "?"
    if base:find("?", 1, true) then separator = base:sub(-1) == "?" and "" or "&" end
    return base .. separator .. table.concat(values, "&") .. suffix
end

return M
