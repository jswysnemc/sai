local loop = require("loop")
local progress = require("progress")
local report = require("report")
local prompt = require("prompt")
local M = {}

local max_steps = sai.config.max_tool_steps
if max_steps == nil then max_steps = 100 end
assert(type(max_steps) == "number" and max_steps >= 0 and max_steps % 1 == 0
    and max_steps <= math.maxinteger, "max_tool_steps must be a non-negative integer")
local tool_timeout_ms = sai.config.tool_timeout_ms
if tool_timeout_ms == nil then tool_timeout_ms = 90000 end
assert(type(tool_timeout_ms) == "number" and tool_timeout_ms > 0 and tool_timeout_ms % 1 == 0
    and tool_timeout_ms <= math.maxinteger, "tool_timeout_ms must be a positive integer")
local progress_mode = sai.config.progress_mode
if progress_mode == nil then progress_mode = "summary" end
assert(progress_mode == "hidden" or progress_mode == "summary" or progress_mode == "full",
    "progress_mode must be hidden, summary or full")

--- 【输入法调查】【调查入口】校验问题并通过当前宿主目录执行完整调查
--- @param args table 输入法现象与可选目标应用
--- @param ctx table 本次调用的可信上下文与进度通道
--- @return table 诊断报告、用量、问题和回复约束
function M.run(args, ctx)
    local issue = sai.text.trim(args.issue)
    assert(issue ~= "", "missing required argument: issue")
    local target = sai.text.trim(args.target or "")
    local catalog = sai.tools.list()
    local names = sai.json.array()
    for _, tool in ipairs(catalog) do
        names[#names + 1] = tool.name
    end

    -- 【输入法调查】【调查入口】1. 模型与工具目录来自本次调用，业务只维护自己的诊断上下文
    local language = sai.config.language or "zh"
    local display = progress.new(ctx, progress_mode, language, catalog)
    progress.report(display, (language == "zh" and "问题" or "issue")
        .. "=\"" .. report.clip_inline(issue, 80) .. "\"")
    local messages = sai.json.array({
        { role = "system", content = prompt },
        { role = "user", content = report.input_prompt(issue, target ~= "" and target or nil) },
    })
    local result, stats = loop.run(messages, names, max_steps, tool_timeout_ms, display)

    -- 【输入法调查】【调查入口】2. 保留原有结果字段，未指定目标使用 JSON null
    return {
        ok = true,
        kind = "linux_input_method_diagnosis",
        issue = issue,
        target = target ~= "" and target or sai.json.null,
        final_answer = report.strip_preamble(result.content),
        stats = stats,
        output_instruction = report.output_instruction,
    }
end

return M
