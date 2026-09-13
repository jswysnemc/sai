local text = require("text")
local M = {}

--- 【输入法模块】【文件识别】识别磁盘上的输入模块动态库
--- @param lower string 小写路径
--- @return boolean 是否属于输入模块
function M.is_module_file(lower)
    return text.contains_any(lower, {"/immodules/", "im-fcitx", "im-xim", "im-ibus", "im-wayland", "platforminputcontext"})
        and (lower:sub(-3) == ".so" or lower:find(".so.", 1, true) ~= nil)
end

--- 【输入法模块】【目录扫描】只递归相关目录，保持五层、每目录三百项和总量一百二十项限制
--- @param probe table 宿主接口
--- @param directory string 目录
--- @param depth integer 当前层数
--- @param found table 模块集合和计数
--- @return nil
local function scan(probe, directory, depth, found)
    if depth > 5 or found.count >= 120 then return end
    for _, entry in ipairs(probe:directory(directory, 300)) do
        local lower = entry.path:lower()
        if entry.is_dir then
            if text.contains_any(lower, {"gtk", "immodules", "qt", "fcitx", "ibus"}) then scan(probe, entry.path, depth + 1, found) end
        elseif entry.is_file and M.is_module_file(lower) then
            local path = text.redact(entry.path, probe.home)
            if not found.values[path] then found.values[path] = true; found.count = found.count + 1 end
        end
        if found.count >= 120 then break end
    end
end

--- 【输入法模块】【可用模块】扫描原版三个库目录，跨根目录去重后稳定排序
--- @param probe table 宿主接口
--- @return table 模块路径数组
function M.available(probe)
    local found = {values={}, count=0}
    for _, directory in ipairs({"/usr/lib", "/usr/lib64", "/app/lib"}) do scan(probe, directory, 0, found) end
    return text.sorted(found.values, 120)
end

--- 【输入法模块】【缓存解析】保留原版对 GTK 缓存字段及引号的解释
--- @param source string 缓存正文
--- @return table 模块路径、名称和区域数组
function M.parse_cache(source)
    local result = sai.json.array()
    for _, line in ipairs(text.lines(source)) do
        local fields = text.words(line)
        if #fields >= 5 then
            result[#result + 1] = {
                so_path=fields[1]:gsub('^"+', ''):gsub('"+$', ''),
                module_name=fields[2]:gsub('^"+', ''):gsub('"+$', ''),
                locales=fields[5]:gsub('^"+', ''):gsub('"+$', ''),
            }
        end
    end
    return result
end

--- 【输入法模块】【GTK 缓存】读取两个原有缓存位置，不在宿主中固化业务路径
--- @param probe table 宿主接口
--- @return table 缓存条目
function M.cache(probe)
    local result = sai.json.array()
    for _, path in ipairs({"/usr/lib/gtk-3.0/3.0.0/immodules.cache", "/usr/lib/gtk-4.0/4.0.0/immodules.cache"}) do
        for _, entry in ipairs(M.parse_cache(probe:read(path) or "")) do result[#result + 1] = entry end
    end
    return result
end

--- 【输入法模块】【命令目标】按空白、路径分隔符和赋值符拆分完整目标名称
--- @param line string 桌面启动命令
--- @param target string 目标名称
--- @return boolean 是否包含独立目标
function M.mentions_target(line, target)
    for part in sai.text.collapse_whitespace(line):gmatch("[^%s/=]+") do if part == target then return true end end
    return false
end

--- 【输入法模块】【桌面入口】优先用户目录，再检查系统目录中的目标桌面启动行
--- @param probe table 宿主接口
--- @param target string 目标名称
--- @return string|nil 启动命令
function M.desktop_exec(probe, target)
    for _, directory in ipairs({"~/.local/share/applications", "/usr/share/applications"}) do
        for _, entry in ipairs(probe:directory(directory, 1024)) do
            if entry.name:sub(-8) == ".desktop" then
                local source = probe:read(entry.path, 65536)
                if source then
                    local command
                    for _, line in ipairs(text.lines(source)) do
                        if line:sub(1, 5) == "Exec=" then command = line:sub(6); break end
                    end
                    if entry.name:sub(1, -9):lower() == target:lower() or (command and M.mentions_target(command, target)) then
                        return command and text.redact(command, probe.home) or nil
                    end
                end
            end
        end
    end
end

return M
