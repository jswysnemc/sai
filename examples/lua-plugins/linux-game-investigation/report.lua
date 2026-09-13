local M = {}

M.output_instruction = [[这是 Linux 游戏兼容性调查子代理返回的最终报告。

请把 final_report 当作主要依据回复用户。不要重新编造兼容性结论。

回复时保留以下核心信息：
- 红绿灯结论，能不能玩
- 怎么玩
- 注意事项

如果用户问“怎么玩”，必须给出可执行步骤。
如果用户追问“刚才完整报告”，直接复述 final_report。]]

M.finalization_prompt = "<tool_budget_reached>调查调用预算已用尽或即将耗尽。不要再请求工具。请只基于上面的用户问题、系统要求和已执行工具结果输出最终调查报告；缺少证据的地方明确写“不确定”或“缺证据”。</tool_budget_reached>"

--- 【游戏调查】【报告整理】优先保留调查章节，否则移除开头的元话语
--- @param content string 模型最终正文
--- @return string 整理后的报告
function M.strip_preamble(content)
    local trimmed = sai.text.trim(content)
    for _, heading in ipairs({ "## 调查结果", "# 调查结果" }) do
        local first = trimmed:find(heading, 1, true)
        if first then
            return sai.text.trim(trimmed:sub(first))
        end
    end
    local lines, started = {}, false
    for line in (trimmed .. "\n"):gmatch("(.-)\n") do
        line = line:gsub("\r$", "")
        local check = sai.text.trim(line)
        if not started then
            local skip = check == "" or check == "---" or check:find("以下是", 1, true)
                or (check:find("最终报告", 1, true) and #check < 30)
            started = not skip
        end
        if started then
            lines[#lines + 1] = line
        end
    end
    return table.concat(lines, "\n")
end

--- 【游戏调查】【工具摘录】归一化空白并保留原有 6000 字符截断规则
--- @param output string 工具输出
--- @return string 用于模型上下文的摘录
local function compact_output(output)
    local value = sai.text.collapse_whitespace(output)
    if utf8.len(value) > 6000 then
        value = value:sub(1, utf8.offset(value, 5998) - 1) .. "..."
    end
    return value
end

--- 【游戏调查】【结果记录】把真实工具参数和执行结果写入后续模型消息
--- @param name string 工具名称
--- @param arguments string 原始 JSON 参数
--- @param ok boolean 工具是否成功
--- @param output string 工具结果
--- @return string 单项结果记录
function M.tool_result(name, arguments, ok, output)
    return string.format(
        "<tool_result name=\"%s\" ok=\"%s\">\narguments_json:\n```json\n%s\n```\noutput:\n```text\n%s\n```\n</tool_result>",
        name, tostring(ok), sai.text.trim(arguments), compact_output(output)
    )
end

--- 【游戏调查】【观察消息】组合已执行结果，明确它们属于宿主观察而非新用户请求
--- @param results table 本轮工具记录
--- @param steps integer 已执行工具次数
--- @param max_steps integer 业务工具预算
--- @return string 可追加为 user 角色的观察消息
function M.transcript(results, steps, max_steps)
    return string.format(
        "<subagent_tool_transcript>\n说明：以下是宿主已经执行完成的内部工具调用结果，不是新的用户请求。请基于这些观察继续调查；如证据已经足够，请输出最终报告。\ntool_budget: %d/%d\n%s\n</subagent_tool_transcript>",
        steps, max_steps, table.concat(results, "\n")
    )
end

return M
