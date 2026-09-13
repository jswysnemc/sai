local M = {}

--- 【Arch 查询】【HTTP】读取授权来源的 JSON，并保留接口错误信息
--- @param url string 完整请求地址
--- @param user_agent string|nil 原工具的请求标识
--- @param timeout_ms integer|nil 单次请求的毫秒上限，默认 30 秒
--- @return table 接口返回的 JSON 数据
function M.json(url, user_agent, timeout_ms)
    local response = sai.http.request({
        url = url,
        method = "GET",
        headers = {
            accept = "application/json",
            ["user-agent"] = user_agent or "sai-archlinux-query/0.1",
        },
        max_bytes = 4194304,
        timeout_ms = timeout_ms or 30000,
    })
    assert(response.status >= 200 and response.status < 300,
        "Arch API returned HTTP " .. response.status .. " for " .. url)
    return sai.json.decode(response.text)
end

return M
