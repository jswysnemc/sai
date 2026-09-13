local collect = require("collect")
local probes = require("probes")
local processes = require("input_method.processes")
local text = require("text")
local reports = require("report")
local M = {}

--- 【应用探测】【受管启动】复用宿主已有命令工具，后台运行时保留可停止的任务编号
--- @param args table 规范化参数
--- @param probe table 宿主接口
--- @param ctx table 当前调用上下文
--- @return nil
local function launch(args, probe, ctx)
    local target = args.target
    local before = processes.find(probe, target, false)
    local launched, after = nil, before
    if #before == 0 then
        -- 【应用探测】【受管启动】1. 名称已排除路径、选项、赋值和 shell 元字符；实际写入权限仍由宿主工具校验
        launched = sai.json.decode(sai.tools.call("run_command", {
            command="exec " .. target, timeout_seconds=args.launch_timeout_seconds,
            cwd=ctx.workdir, label="Diagnostic launch: " .. target,
        }))
        after = processes.find(probe, target, false)
    end
    local previous, added = {}, sai.json.array()
    for _, pid in ipairs(before) do previous[pid] = true end
    for _, pid in ipairs(after) do if not previous[pid] then added[#added + 1] = pid end end
    local background = launched and launched.mode == "background"
    probe.report.facts.launch_probe = {
        target=target, launched_pid=added[1] or sai.json.null, pids_before=before, pids_after=after, new_pids=added,
        managed_task_id=background and launched.task_id or sai.json.null,
        managed_pid=background and launched.task and launched.task.pid or sai.json.null,
        skipped=#before > 0 and "already_running" or sai.json.null,
    }
    if background then
        probe.report.safety_notes[#probe.report.safety_notes + 1] =
            "The launched application is managed as background task " .. launched.task_id .. "; use background_command action=stop to stop it."
    elseif launched and launched.success == false then
        probe.report.ok = false
        probe:missing_evidence("target launch failed: " .. text.clip(launched.stderr or "", 500))
    end
end

--- 【应用探测】【写入入口】明确执行目标版本或启动采样，保留诊断字段并标明动作
--- @param raw table 包含 probe=version 或 probe=launch 的参数
--- @param ctx table 调用上下文
--- @return table 证据报告及显式探测结果
function M.run(raw, ctx)
    local args, report = collect.prepare(raw)
    assert(type(args.target) == "string" and text.executable_name(args.target), "diagnostic_app_probe requires a safe target command name")
    assert(args.area == "app" or args.area == "input_method", "diagnostic_app_probe requires area=app or area=input_method")
    assert(sai.system.platform == "linux" or sai.system.platform == "macos", "application probes support Linux and macOS only")
    local probe = probes.new(report)
    report.safety_notes = sai.json.array({"diagnostic_app_probe explicitly executes the requested application; this action requires write permission"})
    report.facts["app.probe_kind"] = raw.probe
    if raw.probe == "launch" then launch(args, probe, ctx) end
    collect.collect(args, report, probe, {version=raw.probe == "version"})
    if raw.probe == "version" then
        local output = probe:command("app-version", {target=args.target}, 2)
        reports.log(report, args.target .. " --version", output.stdout)
        reports.check(report, "app.version_probe", output.status == 0 and "ok" or "error",
            "explicit target --version execution", reports.compact(output))
        if output.status ~= 0 then report.ok = false end
    end
    return report
end

return M
