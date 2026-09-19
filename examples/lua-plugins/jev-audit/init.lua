local config = require("config")
local provider = require("provider")
local settings = config.load(sai.config)

--- 【Jev审核】【审核入口】服务错误和无效响应只交还人工，不改用其他模型
--- @param input table 宿主审核事实
--- @param ctx table 可信调用上下文
--- @return table 本次工具操作的审核决定
local function review(input, ctx)
    local ok, result = pcall(provider.review, settings, input, ctx)
    if not ok then
        return {decision = "abstain", reason = "Jev 服务不可用或响应无效，等待人工处理"}
    end
    return result
end

sai.register_permission_audit({review = review})
