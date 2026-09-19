local M = {}

--- 【Jev审核】【配置加载】使用 TypeSafe 官方地址，优先使用宿主选定供应商
--- @param settings table 插件配置及宿主在内存中提供的 provider
--- @return table 不包含其他供应商配置的本次请求参数
function M.load(settings)
    local provider = settings.provider or {}
    assert(type(provider) == "table", "jev-audit.provider must be an object")
    local config = {
        base_url = provider.base_url or settings.base_url or "https://api.typesafe.ai/v1",
        model = provider.model or settings.model or "jev-latest",
        api_key = provider.api_key or settings.api_key or "",
        timeout_seconds = settings.timeout_seconds or 10,
        minimum_probability = settings.minimum_probability or 0.9,
        minimum_confidence = settings.minimum_confidence or 0.8,
    }
    assert(type(config.base_url) == "string" and type(config.model) == "string"
        and type(config.api_key) == "string", "jev-audit provider fields must be strings")
    assert(config.model:match("^jev[%w%-%.]*$"), "jev-audit requires an explicitly selected Jev model")
    assert(math.type(config.timeout_seconds) == "integer" and config.timeout_seconds >= 1
        and config.timeout_seconds <= 10, "jev-audit.timeout_seconds must be between 1 and 10")
    config.base_url = config.base_url:gsub("/+$", "")
    for _, field in ipairs({"minimum_probability", "minimum_confidence"}) do
        local value = config[field]
        assert(type(value) == "number" and value >= 0.5 and value <= 1,
            "jev-audit." .. field .. " must be between 0.5 and 1")
    end
    assert(config.base_url == "https://api.typesafe.ai/v1"
        or config.base_url == "https://api.typesafe.ai/v1/systemone",
        "jev-audit requires the TypeSafe official endpoint")
    return config
end

--- 【Jev审核】【凭据读取】复用既有 TypeSafe 密钥，读取仍受独立环境授权限制
--- @param config table 本次配置
--- @return string 非空凭据；缺失时产生不包含敏感值的错误
function M.credential(config)
    if config.api_key ~= "" then return config.api_key end
    for _, name in ipairs({"TYPESAFE_KEY", "TYPESAFE_API_KEY"}) do
        local ok, value = pcall(sai.env.get, name)
        if ok and type(value) == "string" and value ~= "" then return value end
    end
    error("TypeSafe credential is not configured")
end

return M
