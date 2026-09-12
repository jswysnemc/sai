local values = require("values")
local M = {}

--- 【知识库嵌入】【供应商配置】只使用本包由宿主投影的供应商
--- @param config table 配置
--- @return table|nil 嵌入供应商，未配置时返回 nil
function M.provider(config)
    if sai.text.trim(config.embedding_provider_id) == "" or sai.text.trim(config.embedding_model) == "" then return nil end
    assert(not config.provider_error, config.provider_error)
    return assert(config.provider, "embedding provider is unavailable")
end

--- 【知识库嵌入】【向量请求】按原 OpenAI 兼容协议请求单段文字
--- @param config table 配置
--- @param provider table 当前供应商快照
--- @param text string 查询或文本块
--- @return table 单精度有限向量
function M.embed(config, provider, text)
    local key = sai.text.trim(provider.api_key or "")
    assert(key ~= "", "embedding provider " .. provider.id .. " has no api_key")
    local response = sai.http.request({url=provider.endpoint, method="POST",
        headers={authorization="Bearer " .. key, ["content-type"]="application/json"},
        body=sai.json.encode({model=sai.text.trim(config.embedding_model), input=text}),
        max_bytes=1048576, timeout_ms=math.min(120, config.embedding_timeout_seconds) * 1000})
    if response.status < 200 or response.status >= 300 then
        local body = sai.text.collapse_whitespace(response.text)
        body = body:gsub(key:gsub("(%W)", "%%%1"), "[redacted]")
        error("embedding API error at " .. provider.endpoint .. " (" .. response.status .. "): " .. values.truncate(body, 4096))
    end
    local data = sai.json.decode(response.text)
    local embedding = type(data) == "table" and type(data.data) == "table" and type(data.data[1]) == "table" and data.data[1].embedding
    assert(type(embedding) == "table" and getmetatable(embedding) == getmetatable(sai.json.array()), "embedding response missing data[0].embedding")
    local result = sai.json.array()
    for _, value in ipairs(embedding) do
        if type(value) == "number" then
            value = values.f32(value)
            assert(value == value and math.abs(value) < math.huge, "embedding response contains a non-finite value")
            result[#result + 1] = value
        end
    end
    return result
end

return M
