local M = {}
local user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36"

--- 【网页搜图】【查询参数】按原字段顺序生成百分号编码的查询串
--- @param fields table 顺序排列的键值对
--- @return string 查询字符串
function M.query(fields)
    local result = {}
    for _, field in ipairs(fields) do
        -- 1. 【网页搜图】【表单编码】reqwest 查询参数采用空格加号和保留星号的表单编码
        local encoded = sai.text.url_encode(field[2]):gsub("%%20", "+"):gsub("%%2A", "*"):gsub("~", "%%7E")
        result[#result + 1] = field[1] .. "=" .. encoded
    end
    return table.concat(result, "&")
end

--- 【网页搜图】【搜索请求】精确授权引擎来源，正文受运行时大小和时限限制
--- @param config table 包设置
--- @param url string 完整查询地址
--- @param referer string|nil 原请求来源
--- @return string 成功响应正文
function M.get(config, url, referer)
    local headers = {
        ["user-agent"]=user_agent,
        accept="text/html,application/json,text/javascript,image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
        ["accept-language"]="zh-CN,zh;q=0.9,en;q=0.8",
    }
    if referer and referer ~= "" then headers.referer = referer end
    local response = sai.http.request({url=url, headers=headers, max_bytes=4 * 1024 * 1024,
        timeout_ms=math.min(120, math.max(5, config.timeout_seconds)) * 1000})
    assert(response.status >= 200 and response.status < 300, "image search HTTP error (" .. response.status .. ")")
    return response.text
end

return M
