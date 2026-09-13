local arguments = require("arguments")
local reports = require("report")
local probes = require("probes")
local areas = require("areas")
local input_method = require("input_method.evidence")
local processes = require("input_method.processes")
local text = require("text")
local M = {}

--- 【诊断入口】【系统名称】按原版规则查找 os-release 的精确字段
--- @param source string 文件正文
--- @param key string 字段名
--- @return string|nil 去除双引号的值
function M.os_release_value(source, key)
    for _, line in ipairs(text.lines(source)) do
        local name, value = line:match("^([^=]*)=(.*)$")
        if name == key then return value:gsub('^"+', ''):gsub('"+$', '') end
    end
end

--- 【诊断入口】【Linux 系统事实】采集原环境字段、系统名称和内核信息
--- @param probe table 宿主接口
--- @return nil
local function linux_facts(probe)
    for _, name in ipairs({"SHELL", "TERM", "LANG", "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP", "DESKTOP_SESSION",
        "WAYLAND_DISPLAY", "DISPLAY", "GTK_IM_MODULE", "QT_IM_MODULE", "QT_IM_MODULES", "XMODIFIERS", "SDL_IM_MODULE"}) do
        probe:fact_env("env." .. name, name)
    end
    local source = probe:read("/etc/os-release", 65536)
    if source then probe.report.facts["os.pretty_name"] = M.os_release_value(source, "PRETTY_NAME") end
    local uname = probe:command("system-info", {}, 2).stdout
    if sai.text.trim(uname) ~= "" then probe.report.facts["kernel.uname"] = sai.text.trim(uname) end
end

--- 【诊断入口】【macOS 基础采集】保留系统版本、环境及应用进程检查
--- @param args table 参数
--- @param probe table 宿主接口
--- @return nil
local function macos(args, probe)
    probe:fact_env("env.shell", "SHELL")
    probe:fact_env("env.term", "TERM")
    probe:fact_env("env.lang", "LANG")
    reports.log(probe.report, "sw_vers", probe:command("macos-info", {}, 2).stdout)
    if args.area == "app" or args.area == "input_method" then
        if type(args.target) == "string" then probe:command_exists(args.target); processes.find(probe, args.target, true)
        else probe:missing_evidence("target app was not provided") end
    end
end

--- 【诊断入口】【准备报告】标准化输入并创建兼容报告，不执行系统 I/O
--- @param raw table 用户参数
--- @return table, table 参数及报告
function M.prepare(raw)
    local args = arguments.parse(raw)
    return args, reports.new(args, arguments.platform(args.platform))
end

--- 【诊断入口】【平台分派】以单次宿主封装组合九类 Linux 采集和 macOS 基础采集
--- @param args table 规范参数
--- @param report table 报告
--- @param probe table 宿主接口
--- @param options table|nil 显式探测选项
--- @return table 已填充报告
function M.collect(args, report, probe, options)
    if report.platform == "linux" then
        linux_facts(probe)
        if args.area == "input_method" then input_method.collect(args, probe)
        else areas[args.area](args, probe, options or {}) end
    elseif report.platform == "macos" then macos(args, probe)
    else
        report.ok = false
        reports.check(report, "platform.supported", "error", "only linux and macos are supported by check_issue",
            sai.json.array({sai.system.platform}))
    end
    return report
end

--- 【诊断入口】【只读取证】拒绝旧启动选项，明确指向受权限控制的写入工具
--- @param raw table 用户参数
--- @return table 诊断证据
function M.run(raw)
    local args, report = M.prepare(raw)
    assert(not args.allow_launch_probe,
        "allow_launch_probe=true requires diagnostic_app_probe with probe=launch; check_issue is read-only")
    if report.platform == "unsupported" then return M.collect(args, report, nil) end
    return M.collect(args, report, probes.new(report))
end

return M
