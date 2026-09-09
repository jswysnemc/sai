local loop = require("loop")
local progress = require("progress")
local report = require("report")
local prompt = require("prompt")
local M = {}

local max_steps = sai.config.max_tool_steps
if max_steps == nil then max_steps = 0 end
assert(type(max_steps) == "number" and max_steps >= 0 and max_steps % 1 == 0
    and max_steps <= math.maxinteger, "max_tool_steps must be a non-negative integer")
local progress_mode = sai.config.progress_mode
if progress_mode == nil then progress_mode = "summary" end
assert(progress_mode == "hidden" or progress_mode == "summary" or progress_mode == "full",
    "progress_mode must be hidden, summary or full")

--- 【游戏调查】【调查入口】验证证据工具可用后执行 Lua 调查循环并整理报告
--- @param args table 游戏名称与可选关注点
--- @param ctx table 宿主调用上下文和进度通道
--- @return table 最终报告、用量及回复约束
function M.run(args, ctx)
    local game = sai.text.trim(args.game)
    assert(game ~= "", "missing required argument: game")
    local issue = sai.text.trim(args.issue or "")
    local catalog = sai.tools.list()
    local has_signals = false
    local names = sai.json.array()
    for _, tool in ipairs(catalog) do
        names[#names + 1] = tool.name
        has_signals = has_signals or tool.name == "gather_linux_game_compatibility_signals"
    end
    assert(has_signals, "linux-game-signals plugin is disabled or unavailable")

    -- 【游戏调查】【调查入口】1. 证据工具缺失时不初始化模型，不保留隐藏采集路径
    local display = progress.new(ctx, progress_mode, sai.config.language or "zh", catalog)
    progress.report(display, "Linux 游戏兼容性: " .. game)
    local user_prompt = string.format(
        "用户问题：\n游戏：%s\n关注点：%s\n\n请按系统提示词流程完成调查。第一步必须调用 gather_linux_game_compatibility_signals。最终只输出调查报告。",
        game, issue == "" and "未明确" or issue
    )
    local messages = sai.json.array({
        { role = "system", content = prompt },
        { role = "user", content = user_prompt },
    })
    local result, stats = loop.run(messages, names, max_steps, display)

    -- 【游戏调查】【调查入口】2. 原有公开字段继续由插件一次性返回
    return {
        ok = true,
        kind = "linux_game_compatibility",
        game_query = game,
        final_report = report.strip_preamble(result.content),
        stats = stats,
        output_instruction = report.output_instruction,
    }
end

return M
