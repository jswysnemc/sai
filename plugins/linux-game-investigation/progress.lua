local M = {}

--- 【游戏调查】【进度状态】在调用内保存模式、语言和工具显示名
--- @param ctx table 宿主上下文
--- @param mode string hidden、summary 或 full
--- @param language string 当前显示语言
--- @param catalog table 已授权工具目录
--- @return table 本次调查的进度状态
function M.new(ctx, mode, language, catalog)
    local names = {}
    for _, tool in ipairs(catalog) do
        names[tool.name] = tool.display_name ~= "" and tool.display_name or tool.name
    end
    return { ctx = ctx, mode = mode, language = language, names = names, count = 0 }
end

--- 【游戏调查】【进度输出】遵守单条字节和总条数限制，展示预算不会中断调查
--- @param state table 进度状态
--- @param message string 单条进度
--- @return nil
function M.report(state, message)
    if state.count >= 128 then
        return
    end
    state.count = state.count + 1
    state.ctx.progress(sai.text.clip(message, 900))
end

--- 【游戏调查】【详细进度】缩短超长参数和结果预览，保留可解析的进度结构
--- @param state table 进度状态
--- @param prefix string 宿主识别的进度前缀
--- @param payload table 结构化进度
--- @return nil
local function structured(state, prefix, payload)
    if state.count >= 128 then
        return
    end
    payload.name = sai.text.clip(payload.name, 64)
    local text = prefix .. sai.json.encode(payload)
    local size = 512
    while #text > 4096 do
        if payload.output then
            payload.output = sai.text.clip(payload.output, size)
        end
        if payload.args then
            payload.args = sai.text.clip(payload.args, size)
        end
        size = math.max(1, math.floor(size / 2))
        text = prefix .. sai.json.encode(payload)
    end
    state.count = state.count + 1
    state.ctx.progress(text)
end

--- 【游戏调查】【工具开始】按显示模式报告真实工具调用
--- @param state table 进度状态
--- @param step integer 本次工具序号
--- @param call table 模型给出的工具名称与原始参数
--- @return nil
function M.tool_start(state, step, call)
    if state.mode == "summary" then
        local message = state.language == "zh"
            and string.format("工具 #%d：%s 运行中", step, state.names[call.name] or call.name)
            or string.format("tool #%d: %s running", step, call.name)
        M.report(state, message)
    elseif state.mode == "full" then
        structured(state, "__subtool_call__", { name = call.name, args = call.arguments })
    end
end

--- 【游戏调查】【工具完成】错误状态与真实执行结果保持一致
--- @param state table 进度状态
--- @param step integer 本次工具序号
--- @param name string 工具名称
--- @param ok boolean 工具是否成功
--- @param output string 工具输出
--- @return nil
function M.tool_result(state, step, name, ok, output)
    if state.mode == "summary" then
        local label = ok and "ok" or "error"
        local message = state.language == "zh"
            and string.format("工具 #%d：%s %s", step, state.names[name] or name, label)
            or string.format("tool #%d: %s %s", step, name, label)
        M.report(state, message)
    elseif state.mode == "full" then
        structured(state, "__subtool_result__", { name = name, ok = ok, output = output })
    end
end

return M
