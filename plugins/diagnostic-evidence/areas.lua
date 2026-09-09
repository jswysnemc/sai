local text = require("text")
local reports = require("report")
local processes = require("input_method.processes")
local packages = require("packages")
local M = {}

--- 【领域证据】【用户服务】记录 systemd 用户服务的运行状态
--- @param probe table 宿主接口
--- @param name string 服务名称
--- @return nil
local function service(probe, name)
    local output = probe:command("systemd-user", {service=name}, 2)
    reports.check(probe.report, "systemd_user." .. name .. ".active", output.status == 0 and "ok" or "warn",
        "systemctl --user is-active " .. name, reports.compact(output))
end

--- 【领域证据】【系统检查】检查原版基础调查命令的可用性
--- @param args table 参数
--- @param probe table 宿主接口
--- @return nil
function M.system(args, probe)
    for _, name in ipairs({"systemctl", "journalctl", "loginctl", "ip", "df"}) do probe:command_exists(name) end
end

--- 【领域证据】【应用采集】收集命令、进程、所属包及日志，版本执行通过独立写入入口请求
--- @param args table 参数
--- @param probe table 宿主接口
--- @param options table 显式探测选项
--- @return nil
function M.app(args, probe, options)
    if type(args.target) ~= "string" then probe:missing_evidence("target app was not provided"); return end
    local target = args.target
    local path = probe:command_exists(target)
    processes.find(probe, target, true)
    if path then
        probe.report.facts["app.command_path"] = path
        packages.owner(probe, path)
        if not options.version then
            probe:missing_evidence("target --version was not executed by the read-only collector; use diagnostic_app_probe with probe=version")
            probe.report.recommended_next_probes[#probe.report.recommended_next_probes + 1] = "diagnostic_app_probe probe=version target=" .. target
        end
    end
    probe:recent_logs(args, {target, "error", "failed"})
end

--- 【领域证据】【GPU 摘录】保留原版控制器及驱动信息筛选规则
--- @param source string lspci 输出
--- @param home string|nil 主目录
--- @return table 摘录行
function M.gpu_blocks(source, home)
    local result = sai.json.array()
    for _, line in ipairs(text.lines(source)) do
        if text.contains_any(line:lower(), {"vga", "3d controller", "display controller", "kernel driver in use"}) then
            result[#result + 1] = text.redact(line, home)
            if #result == 80 then break end
        end
    end
    return result
end

--- 【领域证据】【GPU 采集】查询 PCI 设备及 NVIDIA 工具可用性
--- @param args table 参数
--- @param probe table 宿主接口
--- @return nil
function M.gpu(args, probe)
    if probe:command_exists("lspci") then
        local blocks = M.gpu_blocks(probe:command("pci", {}, 4).stdout, probe.home)
        if #blocks > 0 then probe.report.facts["gpu.lspci"] = blocks end
    end
    probe:command_exists("nvidia-smi")
end

--- 【领域证据】【显示采集】收集 Portal、媒体服务、Xwayland、GPU 和相关日志
--- @param args table 参数
--- @param probe table 宿主接口
--- @return nil
function M.display(args, probe)
    for _, name in ipairs({"xdg-desktop-portal.service", "pipewire.service", "wireplumber.service"}) do service(probe, name) end
    processes.find(probe, "Xwayland", true)
    M.gpu(args, probe)
    probe:recent_logs(args, {"portal", "pipewire", "wireplumber", "wayland", "xwayland"})
end

--- 【领域证据】【声音采集】收集 PipeWire 服务、设备状态和相关日志
--- @param args table 参数
--- @param probe table 宿主接口
--- @return nil
function M.audio(args, probe)
    for _, name in ipairs({"pipewire.service", "wireplumber.service", "pipewire-pulse.service"}) do service(probe, name) end
    if probe:command_exists("wpctl") then reports.log(probe.report, "wpctl status", probe:command("audio-status", {}, 3).stdout) end
    probe:recent_logs(args, {"pipewire", "wireplumber", "pulse", "audio"})
end

--- 【领域证据】【包管理采集】检查命令、包数据库锁和近期日志
--- @param args table 参数
--- @param probe table 宿主接口
--- @return nil
function M.package(args, probe)
    for _, name in ipairs({"pacman", "yay", "paru"}) do probe:command_exists(name) end
    probe.report.facts["package.pacman_db_lock_exists"] = probe:exists("/var/lib/pacman/db.lck")
    probe:recent_logs(args, {"pacman", "error", "failed", "warning"})
end

--- 【领域证据】【网络地址遮罩】保留原版基于字段的 IPv4、IPv6 和 MAC 遮罩规则
--- @param source string 命令输出
--- @return string 脱敏网络信息
function M.mask_addresses(source)
    local result = {}
    for _, field in ipairs(text.words(source)) do
        if field:find(".", 1, true) and field:find("%d") then result[#result + 1] = "<ipv4>"
        elseif field:find(":", 1, true) and field:find("[A-Fa-f0-9]") then result[#result + 1] = "<ipv6-or-mac>"
        else result[#result + 1] = field end
    end
    return table.concat(result, " ")
end

--- 【领域证据】【网络采集】查询地址和解析服务，不主动发送连通性探测
--- @param args table 参数
--- @param probe table 宿主接口
--- @return nil
function M.network(args, probe)
    local ip = probe:command_exists("ip")
    local resolver = probe:command_exists("resolvectl")
    probe:command_exists("ping")
    if ip then reports.log(probe.report, "ip -brief addr", M.mask_addresses(probe:command("network-addresses", {}, 3).stdout)) end
    if resolver then reports.log(probe.report, "resolvectl status", probe:command("resolver-status", {}, 3).stdout) end
end

--- 【领域证据】【存储采集】查询文件系统空间和 btrfs 命令可用性
--- @param args table 参数
--- @param probe table 宿主接口
--- @return nil
function M.storage(args, probe)
    if probe:command_exists("df") then reports.log(probe.report, "df -hT", probe:command("disk-usage", {}, 3).stdout) end
    probe:command_exists("btrfs")
end

return M
