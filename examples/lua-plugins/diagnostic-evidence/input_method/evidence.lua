local processes = require("input_method.processes")
local modules = require("input_method.modules")
local environment = require("input_method.environment")
local classify = require("input_method.classify")
local paths = require("input_method.paths")
local packages = require("packages")
local reports = require("report")
local M = {}

--- 【输入法取证】【完整流程】组合协议、磁盘、进程、区域和包证据，保留原报告结构
--- @param args table 标准化诊断参数
--- @param probe table 当前调用的受限宿主接口
--- @return nil
function M.collect(args, probe)
    -- 【输入法取证】【完整流程】1. 先收集输入法服务和公共模块证据
    for _, name in ipairs({"fcitx5", "ibus-daemon"}) do processes.find(probe, name, true) end
    if probe:command_exists("fcitx5-remote") then
        local output = probe:command("fcitx-status", {}, 2)
        reports.check(probe.report, "input_method.fcitx5_remote", output.status == 0 and "ok" or "warn",
            "fcitx5-remote status probe", reports.compact(output))
    end
    local wayland = environment.wayland(probe)
    local available, cache = modules.available(probe), modules.cache(probe)
    probe.report.facts["input_method.available_modules"] = available
    probe.report.facts["input_method.immodule_cache"] = cache
    if type(args.target) ~= "string" then
        probe:missing_evidence("target app was not provided; cannot check app toolkit, target environment, loaded .so modules, or path status")
        return
    end

    -- 【输入法取证】【完整流程】2. 只观察现有进程，启动探测由明确的写入工具完成
    local target = args.target
    local pids = processes.find(probe, target, true)
    if #pids == 0 then
        probe:missing_evidence("target app " .. target .. " is not running; runtime environment and loaded .so modules are unavailable")
        probe.report.recommended_next_probes[#probe.report.recommended_next_probes + 1] =
            "start " .. target .. ", then rerun check_issue with area=input_method and target=" .. target
    end
    local target_env, loaded = processes.environment(probe, pids[1]), processes.loaded_modules(probe, pids)
    local locale = environment.locale(probe, target_env)
    local socket = environment.sockets(probe, pids)
    local command_line, desktop = processes.command_line(probe, pids[1]), modules.desktop_exec(probe, target)
    local command_path = probe:command_path(target)
    local package = command_path and packages.framework(probe, command_path, target) or nil
    if package then probe.report.facts["input_method.package_probe"] = package end

    -- 【输入法取证】【完整流程】3. Lua 规则解释证据，宿主不包含工具包分类或路径判定
    local evidence = table.concat({target, command_line or "", desktop or "", command_path or "", package or ""}, " ")
    local display = classify.display(evidence, target_env, socket, loaded)
    local profile = {
        toolkit=classify.refine(classify.toolkit(evidence), display), display_mode=display,
        runtime_observed=#pids > 0 and command_line ~= nil, command_line=command_line or sai.json.null,
        desktop_exec=desktop or sai.json.null, target_env=target_env or sai.json.null,
        loaded_input_modules=loaded, available_input_modules=available, immodule_cache=cache,
        wayland_protocol=wayland, locale_info=locale,
    }
    profile.path_status = paths.evaluate(profile)
    probe.report.facts["input_method.profile"] = profile
    probe:recent_logs(args, {target, "fcitx", "ibus", "qt", "gtk", "xwayland"})
end

return M
