local M = {}
local defaults = {
    max_results=5, max_download_mb=4, safe_search=true, vision_screening_enabled=true,
    auto_preview=true, preview_count=1, timeout_seconds=20, language="en",
    cache_dir="~/Pictures/sai/web-images", duckduckgo_base_url="https://duckduckgo.com",
    bing_base_url="https://www.bing.com",
}

--- 【网页搜图】【设置校验】合并包默认值与兼容设置，保留显式 false
--- @param settings table 当前包设置
--- @return table 已验证的设置
function M.load(settings)
    assert(type(settings) == "table", "web-images settings must be an object")
    local config = {}
    for key, value in pairs(defaults) do config[key] = value end
    for key, value in pairs(settings) do
        assert(defaults[key] ~= nil, "unknown web-images setting: " .. tostring(key))
        assert(type(value) == type(defaults[key]), "web-images." .. key .. " has an invalid type")
        config[key] = value
    end
    for _, key in ipairs({"max_results", "preview_count", "timeout_seconds"}) do
        local value = config[key]
        assert(value == value and value < math.huge and value >= 0 and value % 1 == 0,
            "web-images." .. key .. " must be a non-negative integer")
    end
    assert(config.timeout_seconds <= 600, "web-images.timeout_seconds must not exceed 600")
    assert(config.max_download_mb == config.max_download_mb and math.abs(config.max_download_mb) < math.huge,
        "web-images.max_download_mb must be finite")
    assert(config.language == "zh" or config.language == "en", "web-images.language must be zh or en")
    assert(sai.text.trim(config.cache_dir) ~= "", "web-images.cache_dir must not be empty")
    for _, key in ipairs({"duckduckgo_base_url", "bing_base_url"}) do
        config[key] = sai.text.trim(config[key]):gsub("/+$", "")
        local authority = config[key]:match("^https?://([^/]+)")
        assert(authority and not authority:find("@", 1, true) and not config[key]:find("[?#]")
            and not config[key]:find("\\", 1, true) and not config[key]:find("%c"),
            "web-images." .. key .. " must use HTTP(S) without credentials, query or fragment")
    end
    return config
end

return M
