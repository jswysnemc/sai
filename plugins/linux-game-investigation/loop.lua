local progress = require("progress")
local report = require("report")
local statistics = require("stats")
local M = {}

--- 【游戏调查】【模型循环】按请求累计用量，逐项执行工具并在额度耗尽前收尾
--- @param messages table 插件维护的文本消息
--- @param names table 已授权工具名称
--- @param max_steps integer 业务工具上限，0 表示不额外限制
--- @param display table 进度显示状态
--- @return table 模型最终响应
--- @return table 本次调查统计
function M.run(messages, names, max_steps, display)
    local stats = statistics.new()
    local steps, rounds = 0, 0
    local tool_limit = math.min(max_steps > 0 and max_steps or sai.limits.tool_calls, sai.limits.tool_calls)
    while true do
        -- 【游戏调查】【模型循环】1. 为最后一次无工具请求保留模型额度及 256 条消息上限内的收尾空间
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

        -- 【游戏调查】【模型循环】2. 一个模型响应内也逐项检查工具预算，跳过项不计入执行次数
        local transcript = {}
        for _, call in ipairs(result.tool_calls) do
            if steps >= tool_limit then
                transcript[#transcript + 1] = report.tool_result(call.name, call.arguments, false,
                    "tool skipped: game compatibility tool budget reached")
            else
                steps = steps + 1
                progress.tool_start(display, steps, call)
                local ok, output = pcall(sai.tools.call, call.name, call.arguments)
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
