local text = require("text")
local reports = require("report")
local M = {}

--- 【输入法进程】【匹配过滤】排除采集命令、shell 包装和当前宿主进程
--- @param output string pgrep 输出
--- @param name string 目标关键字
--- @param own_pid integer 宿主 PID
--- @return table 稳定排序的匹配行
function M.matches(output, name, own_pid)
    local matches = sai.json.array()
    for _, line in ipairs(text.lines(output)) do
        local lower = line:lower()
        local fields = text.words(line)
        if lower:find(name:lower(), 1, true)
            and not text.contains_any(lower, {"pgrep -af", "pgrep -af --", "/usr/bin/bash -c", "/bin/sh -c"})
            and tonumber(fields[1]) ~= own_pid then matches[#matches + 1] = line end
    end
    table.sort(matches)
    return matches
end

--- 【输入法进程】【PID 提取】只接受符合 u32 范围的数字标识
--- @param lines table 匹配行
--- @return table PID 数组
local function ids(lines)
    local result = sai.json.array()
    for _, line in ipairs(lines) do
        local token = text.words(line)[1]
        local number = token and token:match("^%+?%d+$") and tonumber(token)
        if number and number <= 4294967295 then result[#result + 1] = number end
    end
    return result
end

--- 【输入法进程】【进程采集】查询目标 PID，并可同时记录可见检查
--- @param probe table 受限宿主封装
--- @param name string 目标关键字
--- @param record boolean 是否添加报告检查
--- @return table PID 数组
function M.find(probe, name, record)
    local output = probe:command("processes", {target=name}, 2)
    local matches = M.matches(output.stdout, name, sai.system.process_id)
    if record then
        reports.check(probe.report, "process." .. name .. ".running", #matches == 0 and "unknown" or "ok",
            #matches == 0 and ("no process matching " .. name .. " was found") or ("process matching " .. name .. " is running"),
            sai.json.array(#matches == 0 and {} or {text.clip(table.concat(matches, "\n"), 1000)}))
    end
    return ids(matches)
end

local environment_names = {
    GTK_IM_MODULE=true, QT_IM_MODULE=true, QT_IM_MODULES=true, XMODIFIERS=true, SDL_IM_MODULE=true,
    GLFW_IM_MODULE=true, XDG_SESSION_TYPE=true, WAYLAND_DISPLAY=true, DISPLAY=true, LANG=true, LC_ALL=true, LC_CTYPE=true,
}

--- 【输入法进程】【目标环境】只将原版输入法相关字段带入报告
--- @param probe table 受限宿主封装
--- @param pid integer|nil 目标 PID
--- @return table|nil 目标环境
function M.environment(probe, pid)
    if not pid then return nil end
    local raw = probe:read("/proc/" .. pid .. "/environ")
    if not raw then return nil end
    local result = {}
    for entry in (raw .. "\0"):gmatch("(.-)%z") do
        local key, value = entry:match("^([^=]*)=(.*)$")
        if key and environment_names[key] then result[key] = text.redact(value, probe.home) end
    end
    return result
end

--- 【输入法进程】【命令行】读取 NUL 分隔的参数并脱敏主目录
--- @param probe table 受限宿主封装
--- @param pid integer|nil 目标 PID
--- @return string|nil 命令行文本
function M.command_line(probe, pid)
    if not pid then return nil end
    local raw = probe:read("/proc/" .. pid .. "/cmdline")
    if not raw then return nil end
    local result = {}
    for part in (raw .. "\0"):gmatch("(.-)%z") do if part ~= "" then result[#result + 1] = text.redact(part, probe.home) end end
    return #result > 0 and table.concat(result, " ") or nil
end

--- 【输入法进程】【映射路径】识别 maps 最后一列中的输入模块动态库
--- @param line string 映射行
--- @param home string|nil 主目录
--- @return string|nil 输入模块路径
function M.module_path(line, home)
    local fields = text.words(line)
    local path = fields[#fields]
    if not path then return nil end
    local lower = path:lower()
    local module = text.contains_any(lower, {"/immodules/", "im-fcitx", "im-xim", "im-ibus", "im-wayland", "platforminputcontext", "libibus", "libfcitx"})
    if module and (lower:sub(-3) == ".so" or lower:find(".so.", 1, true)) then return text.redact(path, home) end
end

--- 【输入法进程】【加载模块】读取最多八个 PID，去重后保留八十条动态库证据
--- @param probe table 受限宿主封装
--- @param pids table 目标 PID
--- @return table 运行时模块数组
function M.loaded_modules(probe, pids)
    local modules = {}
    for index, pid in ipairs(pids) do
        if index > 8 then break end
        for _, line in ipairs(text.lines(probe:read("/proc/" .. pid .. "/maps") or "")) do
            local path = M.module_path(line, probe.home)
            if path then modules["pid " .. pid .. ": " .. path] = true end
        end
    end
    return text.sorted(modules, 80)
end

return M
