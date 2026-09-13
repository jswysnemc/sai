local M = {}

--- 【游戏信号】【可选值】只将缺失值转换为 JSON null，保留 false 和空字符串
--- @param value any 可选值
--- @return any 原值或 JSON null
function M.optional(value)
    if value == nil then return sai.json.null end
    return value
end

--- 【游戏信号】【字段读取】从对象读取字段，其他 JSON 类型按缺失处理
--- @param value any JSON 值
--- @param key string 字段名
--- @return any 字段原值或 JSON null
function M.field(value, key)
    if type(value) == "table" then return M.optional(value[key]) end
    return sai.json.null
end

--- 【游戏信号】【网络读取】通过已授权宿主读取页面，保留原 20 秒请求上限
--- @param url string 完整来源地址
--- @return string 成功响应正文，失败状态抛出错误
function M.get(url)
    local response = sai.http.request({
        url = url,
        timeout_ms = 20000,
        max_bytes = 2097152,
        headers = { ["user-agent"] = "sai-linux-game-compatibility/0.1" },
    })
    assert(response.status >= 200 and response.status < 300, "HTTP status " .. response.status)
    return response.text
end

--- 【游戏信号】【JSON 读取】读取并解析来源返回的 JSON
--- @param url string 完整来源地址
--- @return any 原始 JSON 数据，解析失败抛出错误
function M.json(url)
    return sai.json.decode(M.get(url))
end

--- 【游戏信号】【页面候选】按候选顺序读取首个成功页面，并保留全部尝试
--- @param slugs table 排序后的路径名称
--- @param prefix string 固定来源与路径前缀
--- @param suffix string 固定路径后缀
--- @return table 正文、已选 URL 和尝试记录，空正文仍为成功响应
function M.page(slugs, prefix, suffix)
    local attempts = sai.json.array()
    for _, slug in ipairs(slugs) do
        local url = prefix .. slug .. suffix
        local ok, value = pcall(M.get, url)
        attempts[#attempts + 1] = { slug = slug, url = url, ok = ok }
        if ok then return { text = value, url = url, attempts = attempts } end
        attempts[#attempts].error = tostring(value)
    end
    return { attempts = attempts }
end

return M
