local M = {}

--- 【百科查询】【请求参数】为三类查询统一保留旧版 User-Agent 与十秒单次时限
--- @param url string 已编码地址
--- @param maximum integer 最大正文传输和解码字节数
--- @return table 宿主 HTTP 请求参数
function M.options(url, maximum)
    return {method="GET", url=url, timeout_ms=10000, max_bytes=maximum, headers={["user-agent"]="sai/0.1"}}
end

--- 【百科查询】【状态校验】HTTP 错误保持失败，不把明确状态错误当作页面传输回退条件
--- @param response table 宿主响应
--- @return string 正文
function M.body(response)
    assert(response.status >= 200 and response.status < 300, "Moegirlpedia HTTP status " .. response.status)
    return response.text
end

--- 【百科查询】【JSON 请求】读取有界搜索或解析接口响应
--- @param url string 已编码 API 地址
--- @return any 解码后的 JSON 数据
function M.json(url)
    return sai.json.decode(M.body(sai.http.request(M.options(url, 1048576))))
end

return M
