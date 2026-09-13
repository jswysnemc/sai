local M = {}

--- 【ProtonDB】【HTTP】使用只读请求获取有大小限制的 JSON
--- @param url string 授权来源内的完整地址
--- @param headers table|nil 搜索服务所需的请求头
--- @return table 接口数据
function M.json(url, headers)
    headers = headers or {}
    headers["user-agent"] = "sai-protondb-query/0.1"
    headers.accept = "application/json"
    local response = sai.http.request({
        url = url, method = "GET", headers = headers, max_bytes = 4194304,
        timeout_ms = 20000,
    })
    assert(response.status >= 200 and response.status < 300,
        "ProtonDB API returned HTTP " .. response.status)
    return sai.json.decode(response.text)
end

return M
