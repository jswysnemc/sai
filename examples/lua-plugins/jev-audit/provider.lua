local config = require("config")
local policy = require("policy")
local M = {}

--- 【Jev审核】【官方调用】使用结构化问题与概率响应，不调用聊天模型
--- @param settings table 已校验配置
--- @param input table 宿主审核事实
--- @param ctx table 可信调用上下文
--- @return table 允许、拒绝或交还人工的决定
function M.review(settings, input, ctx)
    local endpoint = settings.base_url
    if not endpoint:match("/systemone$") then endpoint = endpoint .. "/systemone" end
    local response = sai.http.request({
        method = "POST", url = endpoint,
        timeout_ms = settings.timeout_seconds * 1000, max_bytes = 16384,
        headers = {
            ["Authorization"] = "Bearer " .. config.credential(settings),
            ["Content-Type"] = "application/json",
        },
        body = sai.json.encode(policy.request(input, ctx, settings.model)),
    })
    assert(response.status >= 200 and response.status < 300, "Jev provider request failed")
    return policy.decision(sai.json.decode(response.text), settings)
end

return M
