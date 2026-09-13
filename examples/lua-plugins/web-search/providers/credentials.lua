local M = {}
local fallbacks = {
    tinyfish = "TINYFISH_API_KEY", tavily = "TAVILY_API_KEY",
    firecrawl = "FIRECRAWL_API_KEY", anysearch = "ANYSEARCH_API_KEY",
}

--- 【网页搜索】【授权环境】只通过公共接口读取清单和用户均已授权的变量
--- @param name string 环境变量名称
--- @return string|nil 非空值；变量缺失或未获授权时返回 nil
local function environment_key(name)
    local ok, value = pcall(sai.env.get, sai.text.trim(name))
    if not ok or value == nil then return nil end
    value = sai.text.trim(value)
    if value ~= "" then return value end
end

--- 【网页搜索】【凭据选择】依次检查显式密钥、授权环境引用和供应商环境变量
--- @param config table 插件自身配置
--- @param provider string 供应商配置前缀
--- @return string 首个可用密钥；缺失时返回不含凭据的错误
function M.key(config, provider)
    -- 1. 【网页搜索】【显式配置】保留非空密钥的优先级，环境引用不由宿主预先展开
    for _, value in ipairs(config[provider .. "_api_keys"]) do
        local key = sai.text.trim(value)
        if key:sub(1, 5) == "$env:" then key = environment_key(key:sub(6)) end
        if key ~= nil and key ~= "" then return key end
    end
    -- 2. 【网页搜索】【可选回退】只尝试对应供应商变量，未授权时不能读取宿主环境
    local fallback = fallbacks[provider]
    local key = fallback and environment_key(fallback)
    assert(key, "missing " .. provider .. " API key or environment grant")
    return key
end

return M
