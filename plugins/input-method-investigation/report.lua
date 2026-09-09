local M = {}

M.output_instruction = "这是 Linux 输入法诊断子代理返回的最终诊断报告。请把 final_answer 当作主要依据回复用户；如果用户追问完整报告，直接复述 final_answer。"
M.finalization_prompt = "<tool_budget_reached>调查调用预算已用尽或即将耗尽。不要再请求工具。请只基于上面的用户问题、系统要求和已执行工具结果输出最终诊断报告；缺少证据的地方明确写“不确定”或“缺证据”。</tool_budget_reached>"

--- 【输入法调查】【问题提示】保留原有诊断步骤与可选目标的默认说明
--- @param issue string 用户现象
--- @param target string|nil 目标软件或进程
--- @return string 本次调查的用户消息
function M.input_prompt(issue, target)
    return string.format(
        "用户输入法问题：\n%s\n\n目标软件：%s\n\n请按照系统提示词中的流程完成诊断。优先调用 check_issue 收集输入法证据；如果目标软件框架或特殊行为不清楚，可以使用 fcitx5_input_method_wiki_qurey、知识库和网络搜索。最终只输出诊断报告。",
        issue, target or "未明确，需从问题中推断"
    )
end

--- 【输入法调查】【报告整理】优先保留问题分析章节，否则移除开头的元话语
--- @param content string 模型最终正文
--- @return string 整理后的诊断报告
function M.strip_preamble(content)
    local trimmed = sai.text.trim(content)
    for _, heading in ipairs({ "## 问题分析", "# 问题分析" }) do
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
                or (check:find("诊断报告", 1, true) and #check < 20)
            started = not skip
        end
        if started then
            lines[#lines + 1] = line
        end
    end
    return table.concat(lines, "\n")
end

--- 【输入法调查】【文本摘录】归一化 Unicode 空白，保留原有字符计数与省略规则
--- @param value string 待处理文本
--- @param max_chars integer 最大字符数
--- @return string 单行文本或末尾带省略号的摘录
function M.clip_inline(value, max_chars)
    value = sai.text.collapse_whitespace(value)
    if utf8.len(value) > max_chars then
        value = value:sub(1, utf8.offset(value, math.max(0, max_chars - 3) + 1) - 1) .. "..."
    end
    return value
end

--- 【输入法调查】【结果记录】把真实工具参数与执行结果写入后续模型消息
--- @param name string 工具名称
--- @param arguments string 原始 JSON 参数
--- @param ok boolean 工具是否成功
--- @param output string 工具返回文本
--- @return string 单项结果记录
function M.tool_result(name, arguments, ok, output)
    return string.format(
        "<tool_result name=\"%s\" ok=\"%s\">\narguments_json:\n```json\n%s\n```\noutput:\n```text\n%s\n```\n</tool_result>",
        name, tostring(ok), sai.text.trim(arguments), M.clip_inline(output, 6000)
    )
end

--- 【输入法调查】【观察消息】组合内部调用记录，明确它们属于宿主观察
--- @param results table 本轮工具记录
--- @param steps integer 已执行工具次数
--- @param max_steps integer 业务工具预算
--- @return string 后续模型请求的 user 消息
function M.transcript(results, steps, max_steps)
    return string.format(
        "<subagent_tool_transcript>\n说明：以下是宿主已经执行完成的内部工具调用结果，不是新的用户请求。请基于这些观察继续诊断；如证据已经足够，请输出最终报告。\ntool_budget: %d/%d\n%s\n</subagent_tool_transcript>",
        steps, max_steps, table.concat(results, "\n")
    )
end

return M
