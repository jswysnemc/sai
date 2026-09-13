local defaults = {
    provider_type="openai", base_url="https://api.openai.com", api_keys={}, model="gpt-image-1",
    default_aspect_ratio="自动", default_resolution="1K", output_dir="~/Pictures/sai/generated-images",
    auto_print=false, timeout_seconds=180,
}
local M = {}

--- 【图片生成】【设置快照】合并包默认值与独立设置，显式 false 不被默认值覆盖
--- @param settings table 插件自己的设置
--- @return table 完成校验的配置
function M.load(settings)
    assert(type(settings) == "table", "image-generation settings must be an object")
    local config = {}
    for name, value in pairs(defaults) do config[name] = value end
    for name, value in pairs(settings) do
        assert(defaults[name] ~= nil, "unknown image-generation setting: " .. tostring(name))
        assert(type(value) == type(defaults[name]), "image-generation." .. name .. " has an invalid type")
        config[name] = value
    end
    for index, value in pairs(config.api_keys) do
        assert(math.type(index) == "integer" and index >= 1 and index <= #config.api_keys and type(value) == "string",
            "image-generation.api_keys must be an array of strings")
    end
    assert(math.type(config.timeout_seconds) == "integer" and config.timeout_seconds >= 1 and config.timeout_seconds <= 600,
        "image-generation.timeout_seconds must be an integer between 1 and 600")
    -- 1. 【图片生成】【地址校验】设置只选择地址，不派生网络权限
    config.base_url = sai.text.trim(config.base_url):gsub("/+$", "")
    local authority = config.base_url:match("^https?://([^/]+)")
    assert(authority and not authority:find("@", 1, true) and not config.base_url:find("[?#]")
        and not config.base_url:find("\\", 1, true) and not config.base_url:find("%c"),
        "image-generation.base_url must use HTTP(S) without credentials, query or fragment")
    -- 2. 【图片生成】【目录校验】父级跳转在配置阶段拒绝，实际写入仍检查独立授权
    assert(not config.output_dir:find("%c"), "image-generation.output_dir must not contain control characters")
    for segment in config.output_dir:gmatch("[^/\\]+") do
        assert(segment ~= "..", "image-generation.output_dir must not contain parent traversal")
    end
    return config
end

return M
