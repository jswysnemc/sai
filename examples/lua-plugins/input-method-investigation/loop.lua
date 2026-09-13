local progress = require("progress")
local report = require("report")
local statistics = require("stats")
local M = {}

--- 【输入法调查】【模型循环】执行工具、控制单次超时，并为最终诊断预留调用和消息额度
--- @param messages table 本次调查维护的文本消息
--- @param names table 已授权的工具名称
--- @param max_steps integer 业务工具上限，0 表示不额外限制
--- @param tool_timeout_ms integer 单次工具时限，单位毫秒
--- @param display table 进度显示状态
--- @return table 最终模型响应
--- @return table 本次调查统计
function M.run(messages, names, max_steps, tool_timeout_ms, display)
    local stats = statistics.new()
    local steps, rounds = 0, 0
    local tool_limit = math.min(max_steps > 0 and max_steps or sai.limits.tool_calls, sai.limits.tool_calls)
    while true do
        -- 【输入法调查】【模型循环】1. 为无工具收尾保留模型请求与 256 条消息内的剩余空间
        local finalizing = steps >= tool_limit or rounds >= sai.limits.model_requests - 1
            or #messages >= 254
        if finalizing then
            messages[#messages + 1] = { role = "user", content = report.finalization_prompt }
        end
        local result = sai.model.complete({
            messages = messages,
            tools = finalizing and sai.json.array() or names,
            stream_reasoning = display.mode ~= "hidden",
        })
        rounds = rounds + 1
        statistics.add_response(stats, result, messages)
        if finalizing or #result.tool_calls == 0 then
            return result, statistics.public(stats)
        end
        if sai.text.trim(result.content) ~= "" then
            messages[#messages + 1] = { role = "assistant", content = result.content, reasoning = result.reasoning }
        end

        -- 【输入法调查】【模型循环】2. 超时和执行错误作为观察继续交给模型，只有实际尝试计入统计
        local transcript = {}
        for _, call in ipairs(result.tool_calls) do
            if steps >= tool_limit then
                transcript[#transcript + 1] = report.tool_result(call.name, call.arguments, false,
                    "tool skipped: input method diagnosis tool budget reached")
            else
                steps = steps + 1
                progress.tool_start(display, steps, call)
                local ok, output = pcall(sai.tools.call, call.name, call.arguments,
                    { timeout_ms = tool_timeout_ms })
                if not ok then
                    output = "tool error: " .. tostring(output)
                end
                statistics.add_tool(stats, ok)
                progress.tool_result(display, steps, call.name, ok, output)
                transcript[#transcript + 1] = report.tool_result(call.name, call.arguments, ok, output)
            end
        end
        if #transcript > 0 then
            messages[#messages + 1] = {
                role = "user",
                content = report.transcript(transcript, steps, max_steps),
            }
        end
    end
end

return M
