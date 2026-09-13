local text = require("text")
local reports = require("report")
local settings = require("settings")
local M = {}

--- 【诊断包信息】【所属包】从原版支持的英文或中文 pacman 输出中提取名称
--- @param source string 包归属输出
--- @return string|nil 包名
function M.owner_name(source)
    local fields = text.words(source)
    for index, value in ipairs(fields) do if value == "by" or value == "由" then return fields[index + 1] end end
end

--- 【诊断包信息】【文件筛选】只保留与应用框架和启动入口相关的包文件
--- @param line string 包文件行
--- @return boolean 是否应保留
function M.relevant_line(line)
    local lower = line:lower()
    return text.contains_any(lower, {"libgtk", "libgdk", "libqt", "platforminputcontext", "immodules", "electron", "chrome", "/bin/"})
        or lower:sub(-8) == ".desktop"
end

--- 【诊断包信息】【归属日志】在 pacman 可用时保留目标文件归属证据
--- @param probe table 宿主接口
--- @param path string 真实命令路径
--- @return nil
function M.owner(probe, path)
    if not probe:command_path("pacman") then return end
    reports.log(probe.report, "pacman -Qo", probe:command("package-owner", {path=path}, 3).stdout)
end

--- 【诊断包信息】【框架采集】有时限地查询包归属和文件，替代原同步无限等待
--- @param probe table 宿主接口
--- @param path string 命令路径
--- @param target string 目标名称
--- @return string|nil 包名与相关文件摘要
function M.framework(probe, path, target)
    local owner = probe:command("package-owner", {path=path}, 3)
    if owner.status ~= 0 then return nil end
    local package = M.owner_name(owner.stdout)
    if not package or not text.executable_name(package) then return nil end
    local output = probe:command("package-files", {package=package}, 3)
    if output.status ~= 0 then return nil end
    local lines = {"package=" .. package, "target=" .. target}
    for _, line in ipairs(text.lines(output.stdout)) do
        if M.relevant_line(line) then lines[#lines + 1] = line end
        if #lines >= 82 then break end
    end
    return text.redact(text.clip(table.concat(lines, "\n"), math.min(settings.max_stdout_chars, 4000)), probe.home)
end

return M
