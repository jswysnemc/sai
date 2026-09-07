local parser = require("status_parser")
local format = require("status_format")

--- 【服务状态】【查询】读取官方页面并输出稳定的状态结构
--- @param args table 包含 include_incidents 和 max_incidents
--- @return table 服务汇总、组件状态和近期事件
local function query(args)
    local response = sai.http.request({
        url = "https://status.deepseek.com/",
        headers = { accept = "text/html" },
        max_bytes = 4194304,
    })
    assert(response.status >= 200 and response.status < 300, "DeepSeek status HTTP status " .. response.status)
    local raw = parser.parse(response.text)
    local limit = math.max(1, math.min(20, args.max_incidents and args.max_incidents >= 0 and args.max_incidents or 5))
    return format.response(raw, args.include_incidents ~= false, limit)
end

sai.register_tool({
    name = "query_deepseek_status",
    description = "Query DeepSeek service status from the official status page.",
    parameters = {
        type = "object",
        properties = {
            include_incidents = { type = "boolean", description = "Whether to include recent incidents, default true." },
            max_incidents = { type = "integer", description = "Maximum recent incidents to return, 1-20, default 5." },
        },
        additionalProperties = false,
    },
    execute = query,
})
