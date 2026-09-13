local M = {}

--- 【知识库嵌入】【独立设置】校验本包供应商，不读取应用供应商配置
--- @param input table 端点、显式密钥与环境变量名
--- @param id string 用于沿用语义索引的供应商标识
--- @return table 完整设置，环境凭据尚未展开
function M.validate(input, id)
    assert(type(input) == "table" and getmetatable(input) ~= getmetatable(sai.json.array()),
        "knowledge-base.provider must be an object")
    local result = {id=id, endpoint="https://api.openai.com/v1/embeddings", api_key="", api_key_env="SAI_KB_EMBEDDING_API_KEY"}
    for key, value in pairs(input) do
        assert(result[key] ~= nil and type(value) == "string", "invalid knowledge-base.provider field")
        result[key] = value
    end
    assert(result.id == id, "provider.id must match embedding_provider_id")
    result.endpoint = sai.text.trim(result.endpoint)
    local authority = result.endpoint:match("^https?://([^/]+)")
    assert(authority and not authority:find("@", 1, true) and not result.endpoint:find("[?#]")
        and not result.endpoint:find("\\", 1, true) and not result.endpoint:find("[%c%s]"),
        "embedding endpoint must use HTTP(S) without credentials, query or fragment")
    assert(result.api_key_env == "" or result.api_key_env:match("^[%a_][%w_]*$"), "invalid embedding environment name")
    return result
end

--- 【知识库嵌入】【调用凭据】优先使用显式密钥，其次读取当前已授权环境变量
--- @param config table 已完成设置校验的插件配置
--- @return table|nil 单次调用的供应商快照，未选择模型时为空
function M.resolve(config)
    if sai.text.trim(config.embedding_provider_id) == "" or sai.text.trim(config.embedding_model) == "" then return nil end
    local provider = config.provider
    local key = sai.text.trim(provider.api_key)
    if key == "" and provider.api_key_env ~= "" then
        local ok, value = pcall(sai.env.get, provider.api_key_env)
        if ok and type(value) == "string" then key = sai.text.trim(value) end
    end
    assert(key ~= "", "embedding provider " .. provider.id .. " has no api_key or environment grant")
    return {id=provider.id, endpoint=provider.endpoint, api_key=key}
end

return M
