local M = {}

--- 【天气查询】【当前天气】保留地点编码、自动定位和原有纯文本结果格式
--- @param args table 可选 location 地点
--- @return string 当前天气文本；空响应或 HTTP 错误时失败
function M.run(args)
    local location = sai.text.trim(args.location or "")
    local path = location == "" and "" or "/" .. sai.text.url_encode(location)
    local response = sai.http.request({
        method="GET", url="https://wttr.in" .. path .. "?format=%C+%t+%w+%l",
        timeout_ms=30000, max_bytes=65536,
    })
    assert(response.status >= 200 and response.status < 300, "weather HTTP status " .. response.status)
    local body = sai.text.trim(response.text)
    assert(body ~= "", "weather response was empty")
    return "current weather(condition,temperature,wind,location): " .. body
end

return M
