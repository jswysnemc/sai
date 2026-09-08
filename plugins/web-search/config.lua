local M = {}

local defaults = {
    default_provider = "auto", max_results = 5, timeout_seconds = 20,
    tinyfish_enabled = true, tinyfish_api_keys = {},
    tinyfish_base_url = "https://api.search.tinyfish.ai",
    tinyfish_default_location = "", tinyfish_default_language = "",
    tavily_enabled = true, tavily_api_keys = {},
    tavily_base_url = "https://api.tavily.com/search", tavily_search_depth = "basic",
    tavily_include_answer = false, tavily_include_raw_content = true,
    firecrawl_enabled = true, firecrawl_api_keys = {},
    firecrawl_base_url = "https://api.firecrawl.dev/v2/search", firecrawl_only_main_content = true,
    anysearch_enabled = true, anysearch_api_keys = {},
    anysearch_base_url = "https://api.anysearch.com/v1/search",
    searxng_enabled = true, searxng_base_url = "", searxng_language = "auto", searxng_safe_search = 0,
    duckduckgo_enabled = true,
}

M.order = { "tinyfish", "tavily", "firecrawl", "anysearch", "searxng", "duckduckgo" }

--- 【网页搜索】【开关检查】沿用供应商开关，未设置地址的 SearXNG 不参与查询
--- @param config table 合并后的搜索设置
--- @param provider string 供应商名称，允许旧版 script 别名
--- @return boolean 是否允许尝试该供应商
function M.enabled(config, provider)
    if provider == "script" then provider = "duckduckgo" end
    if provider == "searxng" and sai.text.trim(config.searxng_base_url) == "" then return false end
    return config[provider .. "_enabled"] == true
end

--- 【网页搜索】【数值校验】验证配置使用指定范围内的整数
--- @param config table 搜索设置
--- @param name string 配置字段名称
--- @param minimum integer 最小值
--- @param maximum integer 最大值
--- @return nil 配置非法时抛出不包含字段值的错误
local function integer_range(config, name, minimum, maximum)
    local value = config[name]
    assert(math.type(value) == "integer" and value >= minimum and value <= maximum,
        "web-search." .. name .. " must be an integer between " .. minimum .. " and " .. maximum)
end

--- 【网页搜索】【配置加载】建立插件设置快照并验证字段类型和当前默认供应商
--- @param settings table 用户配置或宿主提供的兼容设置
--- @return table 可用于所有供应商的设置
function M.load(settings)
    -- 1. 【网页搜索】【设置合并】显式设置覆盖包内默认值
    local config = {}
    for name, value in pairs(defaults) do config[name] = value end
    for name, value in pairs(settings) do config[name] = value end
    -- 2. 【网页搜索】【设置校验】错误只说明字段和契约，不输出可能包含凭据的值
    for name, value in pairs(defaults) do
        assert(type(config[name]) == type(value), "web-search." .. name .. " has an invalid type")
        if type(value) == "table" then
            for index, key in pairs(config[name]) do
                assert(math.type(index) == "integer" and index >= 1 and index <= #config[name]
                    and type(key) == "string", "web-search." .. name .. " must contain only strings")
            end
        end
    end
    integer_range(config, "max_results", 1, 10)
    integer_range(config, "timeout_seconds", 1, 120)
    integer_range(config, "searxng_safe_search", 0, 2)
    assert(config.tavily_search_depth == "basic" or config.tavily_search_depth == "advanced",
        "web-search.tavily_search_depth must be basic or advanced")
    local provider = config.default_provider
    local known = provider == "auto" or provider == "script"
    for _, name in ipairs(M.order) do known = known or name == provider end
    assert(known, "web-search.default_provider is invalid")
    assert(provider == "auto" or M.enabled(config, provider), "web-search.default_provider is disabled")
    return config
end

return M
