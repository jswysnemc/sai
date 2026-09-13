local settings = require("config")
local config = settings.load(sai.config)
local providers = {
    tinyfish = require("providers.tinyfish"),
    tavily = require("providers.tavily"),
    firecrawl = require("providers.firecrawl"),
    anysearch = require("providers.anysearch"),
    searxng = require("providers.searxng"),
    duckduckgo = require("providers.duckduckgo"),
}

local M = {}

--- 【网页搜索】【参数回退】仅使用非空字符串覆盖配置，保留 Unicode 空白处理
--- @param value any 可选工具参数
--- @param fallback string 配置中的默认文本
--- @return string 去除两端空白后的有效文本
local function optional_text(value, fallback)
    if type(value) == "string" and sai.text.trim(value) ~= "" then return sai.text.trim(value) end
    return sai.text.trim(fallback)
end

--- 【网页搜索】【请求路由】按原有顺序尝试已启用供应商并返回首个成功结果
--- @param args table 查询、供应商、结果数量和可选地域参数
--- @return string 搜索结果 Markdown，全部失败时抛出统一错误
function M.run(args)
    -- 1. 【网页搜索】【输入准备】统一查询词、数量和可选地域参数
    local query = sai.text.trim(args.query or "")
    assert(query ~= "", "query is required")
    local count = args.max_results
    if math.type(count) ~= "integer" or count < 0 then count = config.max_results end
    local input = {
        query = query,
        max_results = math.max(1, math.min(count, 10)),
        location = optional_text(args.location, config.tinyfish_default_location),
        language = optional_text(args.language, config.tinyfish_default_language),
    }
    local requested = args.provider or config.default_provider
    if requested == "script" then requested = "duckduckgo" end
    local order = {}
    -- 2. 【网页搜索】【供应商过滤】保留固定顺序，仅选择已启用的目标
    for _, name in ipairs(settings.order) do
        if (requested == "auto" or requested == name) and settings.enabled(config, name) then
            order[#order + 1] = name
        end
    end
    assert(#order > 0, "web search provider is disabled or unknown: " .. requested)
    -- 3. 【网页搜索】【失败回退】单个请求失败不丢失后续供应商的查询机会
    for _, name in ipairs(order) do
        local ok, output = pcall(providers[name], input, config)
        if ok and type(output) == "string" and sai.text.trim(output) ~= "" then return output end
    end
    error("no enabled web search provider succeeded")
end

return M
