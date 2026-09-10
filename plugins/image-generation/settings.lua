local defaults = {
    provider_type="openai", base_url="https://api.openai.com", api_keys={}, model="gpt-image-1",
    default_aspect_ratio="自动", default_resolution="1K", output_dir="~/Pictures/sai/generated-images",
    auto_print=false, timeout_seconds=180,
}
local M = {}

--- 【图片生成】【设置快照】合并包默认值与宿主兼容设置，显式 false 不被默认值覆盖
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
    return config
end

return M
