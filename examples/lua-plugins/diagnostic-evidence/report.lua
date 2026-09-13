local text = require("text")
local M = {}

--- 【诊断报告】【初始结构】保留原工具的字段与空数组类型
--- @param args table 规范化参数
--- @param platform string 实际采集平台
--- @return table 待填充的证据报告
function M.new(args, platform)
    return {
        ok=true, kind="diagnostic_evidence", platform=platform, query=args.query, area=args.area,
        target=args.target, symptom=args.symptom, depth=args.depth, facts={},
        checks=sai.json.array(), logs=sai.json.array(), missing_evidence=sai.json.array(),
        safety_notes=sai.json.array({"check_issue uses fixed read-only probes and does not diagnose or apply fixes"}),
        recommended_next_probes=sai.json.array(),
    }
end

--- 【诊断报告】【检查记录】添加带状态与证据的单项检查
--- @param report table 报告
--- @param id string 稳定标识
--- @param status string 检查状态
--- @param detail string 描述
--- @param evidence table|nil 证据数组
--- @return nil
function M.check(report, id, status, detail, evidence)
    report.checks[#report.checks + 1] = {id=id, status=status, detail=detail, evidence=evidence or sai.json.array()}
end

--- 【诊断报告】【日志摘录】去除空正文并沿用两千字符摘要上限
--- @param report table 报告
--- @param source string 来源
--- @param value string 正文
--- @return nil
function M.log(report, source, value)
    if sai.text.trim(value) ~= "" then report.logs[#report.logs + 1] = {source=source, message=text.clip(value, 2000)} end
end

--- 【诊断报告】【命令摘要】保留退出状态、标准输出、错误输出和超时标记
--- @param output table 进程结果
--- @return table 兼容原格式的摘要数组
function M.compact(output)
    local evidence = sai.json.array()
    if type(output.status) == "number" then evidence[#evidence + 1] = "exit=" .. output.status end
    if sai.text.trim(output.stdout) ~= "" then evidence[#evidence + 1] = "stdout=" .. text.clip(output.stdout, 800) end
    if sai.text.trim(output.stderr) ~= "" then evidence[#evidence + 1] = "stderr=" .. text.clip(output.stderr, 800) end
    if output.timed_out then evidence[#evidence + 1] = "timed_out=true" end
    return evidence
end

return M
