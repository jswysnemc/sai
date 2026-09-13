local M = {}

--- 【游戏调查】【统计初始化】创建一次调查的独立统计状态
--- @return table 工具计数、供应商用量和估算方式
function M.new()
    return {
        tool_calls = 0, tool_ok = 0, tool_errors = 0,
        prompt_tokens = 0, completion_tokens = 0, total_tokens = 0,
        token_estimate = 0, method = "none",
    }
end

--- 【游戏调查】【模型用量】每个响应只累计一次，缺失用量时估算该请求的输入输出
--- @param stats table 当前统计
--- @param result table 单次模型响应
--- @param messages table 本次实际发送的消息
--- @return nil
function M.add_response(stats, result, messages)
    local usage = result.usage
    if type(usage) == "table" and usage.total_tokens > 0 then
        stats.prompt_tokens = stats.prompt_tokens + usage.prompt_tokens
        stats.completion_tokens = stats.completion_tokens + usage.completion_tokens
        stats.total_tokens = stats.total_tokens + usage.total_tokens
        stats.token_estimate = stats.token_estimate + usage.total_tokens
        stats.method = (stats.method == "none" or stats.method == "provider_usage")
            and "provider_usage" or "provider_usage_plus_estimate"
        return
    end
    local inputs = {}
    for _, message in ipairs(messages) do
        inputs[#inputs + 1] = message.content
    end
    local reasoning = type(result.reasoning) == "string" and result.reasoning or ""
    local output = result.content .. reasoning .. sai.json.encode(result.tool_calls)
    stats.token_estimate = stats.token_estimate
        + sai.text.estimate_tokens(table.concat(inputs))
        + sai.text.estimate_tokens(output)
    stats.method = (stats.method == "none" or stats.method == "rough_char_estimate")
        and "rough_char_estimate" or "provider_usage_plus_estimate"
end

--- 【游戏调查】【工具计数】只有实际尝试的调用计入统计，失败单独累计
--- @param stats table 当前统计
--- @param ok boolean 本次工具是否成功
--- @return nil
function M.add_tool(stats, ok)
    stats.tool_calls = stats.tool_calls + 1
    if ok then
        stats.tool_ok = stats.tool_ok + 1
    else
        stats.tool_errors = stats.tool_errors + 1
    end
end

--- 【游戏调查】【公开统计】保持原有结果字段并明确估算与实际用量
--- @param stats table 当前统计
--- @return table 可序列化的统计结果
function M.public(stats)
    return {
        tool_calls = stats.tool_calls,
        tool_ok = stats.tool_ok,
        tool_errors = stats.tool_errors,
        prompt_tokens = stats.prompt_tokens,
        completion_tokens = stats.completion_tokens,
        total_tokens = stats.total_tokens,
        token_estimate = stats.token_estimate,
        token_estimate_method = stats.method == "none" and "rough_char_estimate" or stats.method,
        token_estimate_is_actual = stats.method == "provider_usage",
    }
end

return M
