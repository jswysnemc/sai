local text = require("text")
local reports = require("report")
local processes = require("input_method.processes")
local M = {}

--- 【输入法环境】【协议采集】查询合成器协议及 fcitx5 已加载的 Wayland 前端
--- @param probe table 宿主接口
--- @return table 协议证据
function M.wayland(probe)
    local output = probe:command("wayland-info", {}, 3)
    local supports = output.stdout:find("zwp_text_input_manager_v3", 1, true) ~= nil
    reports.check(probe.report, "input_method.wayland_text_input_v3", supports and "ok" or "unknown",
        "wayland-info: compositor text-input-v3 protocol support", reports.compact(output))
    local pids = processes.find(probe, "fcitx5", false)
    local maps = pids[1] and probe:read("/proc/" .. pids[1] .. "/maps") or ""
    return {
        compositor_supports_text_input_v3=supports, wayland_info_available=type(output.status) == "number",
        fcitx5_wayland_frontend_loaded=text.contains_any(maps or "", {"libwaylandim.so", "libwayland.so"}),
    }
end

--- 【输入法环境】【区域解释】依据目标进程而非宿主 shell 判断区域是否可用
--- @param environment table|nil 目标环境
--- @param source string locale -a 输出
--- @return table 区域证据
function M.locale_info(environment, source)
    local env = type(environment) == "table" and environment or {}
    local lang, ctype = env.LANG, env.LC_CTYPE or env.LC_ALL
    local available = sai.json.array()
    for _, line in ipairs(text.lines(source)) do
        line = sai.text.trim(line)
        if line ~= "" then available[#available + 1] = line end
    end
    local value, valid = ctype or lang or "C", false
    if value ~= "C" and value ~= "POSIX" then
        for _, item in ipairs(available) do
            if item == value or item:lower() == value:lower() or item:match("^[^.]*") == value:match("^[^.]*") then valid = true end
        end
    end
    return {target_lang=lang or sai.json.null, target_lc_ctype=ctype or sai.json.null, available_locales=available, locale_valid=valid}
end

--- 【输入法环境】【区域采集】查询已安装区域并记录原版事实字段
--- @param probe table 宿主接口
--- @param environment table|nil 目标环境
--- @return table 区域证据
function M.locale(probe, environment)
    local result = M.locale_info(environment, probe:command("locales", {}, 2).stdout)
    probe.report.facts["input_method.available_locales"] = result.available_locales
    return result
end

--- 【输入法环境】【套接字关联】按真实 inode 列和完整 PID 关联 X11 与 Wayland 套接字
--- @param socket_text string ss -xp 输出
--- @param unix_text string /proc/net/unix 正文
--- @param pids table 目标 PID
--- @return string, table 显示模式和套接字事实
function M.socket_mode(socket_text, unix_text, pids)
    local x11, wayland, selected = {}, {}, {}
    for index, pid in ipairs(pids) do if index <= 8 then selected[pid] = true end end
    for _, line in ipairs(text.lines(unix_text)) do
        local fields = text.words(line)
        -- 【输入法环境】【套接字关联】1. /proc/net/unix 的第七列是 inode，第八列是路径
        if fields[7] and line:find("X11-unix", 1, true) then x11[fields[7]] = true end
        if fields[7] and line:find("wayland", 1, true) then wayland[fields[7]] = true end
    end
    local has_x11, has_wayland = false, false
    for _, line in ipairs(text.lines(socket_text)) do
        local matches = false
        for pid in line:gmatch("pid=(%d+)") do if selected[tonumber(pid)] then matches = true end end
        if matches then
            for _, field in ipairs(text.words(line)) do
                if field:match("^%d+$") then
                    has_x11 = has_x11 or x11[field] == true
                    has_wayland = has_wayland or wayland[field] == true
                end
            end
        end
    end
    return has_x11 and "x_wayland" or (has_wayland and "wayland_native" or "unknown"),
        {has_x11_socket=has_x11, has_wayland_socket=has_wayland}
end

--- 【输入法环境】【显示采集】没有目标进程时不探测套接字
--- @param probe table 宿主接口
--- @param pids table 目标 PID
--- @return string 显示模式
function M.sockets(probe, pids)
    if #pids == 0 then return "unknown" end
    local mode, facts = M.socket_mode(probe:command("sockets", {}, 3).stdout, probe:read("/proc/net/unix") or "", pids)
    probe.report.facts["input_method.socket_display_mode"] = facts
    return mode
end

return M
