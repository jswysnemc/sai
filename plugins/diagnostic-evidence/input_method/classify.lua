local text = require("text")
local M = {}

--- 【输入法分类】【工具包识别】保留原版 Qt、Electron、GTK 等证据的优先顺序
--- @param value string 目标、命令、桌面入口和包文件证据
--- @return string 工具包名称
function M.toolkit(value)
    local lower = value:lower()
    for _, rule in ipairs({
        {"qt", {"qt_im_module", "platforminputcontext", "libqt"}},
        {"electron_chromium", {"electron", "chromium", "chrome-sandbox", "steamwebhelper", "--ozone-platform", "linuxqq"}},
        {"gtk", {"gtk", "gdk", "immodules"}}, {"sdl", {"sdl"}}, {"java", {"java"}}, {"x11_legacy", {"x11", "xlib"}},
    }) do
        if text.contains_any(lower, rule[2]) then return rule[1] end
    end
    return "unknown"
end

--- 【输入法分类】【显示模式】启动参数优先，其后采用套接字、已加载模块和目标环境
--- @param value string 命令与包信息
--- @param env table|nil 目标环境
--- @param socket string 套接字证据
--- @param loaded table 已加载模块
--- @return string 显示模式
function M.display(value, env, socket, loaded)
    if value:lower():find("--ozone-platform=wayland", 1, true) then return "wayland_native" end
    if socket == "x_wayland" or socket == "x11" then return "x_wayland" end
    if socket == "wayland_native" then return socket end
    for _, module in ipairs(loaded) do
        if module:lower():find("im-wayland", 1, true) then return "wayland_native" end
    end
    if type(env) == "table" then
        local wayland, display = env.WAYLAND_DISPLAY ~= nil, env.DISPLAY ~= nil
        if wayland and display then return "x_wayland" end
        if wayland then return "wayland_native" end
        if display then return "x11" end
    end
    return "unknown"
end

--- 【输入法分类】【Electron 细分】依显示模式保持原版适配路径选择
--- @param toolkit string 初始工具包
--- @param display string 显示模式
--- @return string 细分后的工具包
function M.refine(toolkit, display)
    if toolkit ~= "electron_chromium" then return toolkit end
    return display == "wayland_native" and "electron_wayland" or "electron_x11"
end

--- 【输入法分类】【路径集合】保持不同工具包需要检查的输入路径及原有顺序
--- @param toolkit string 工具包名称
--- @return table 输入路径数组
function M.relevant_paths(toolkit)
    local paths = {
        gtk={"wayland_protocol", "toolkit_module", "xim"}, qt={"wayland_protocol", "toolkit_module", "xim"},
        sdl={"wayland_protocol", "toolkit_module", "xim"}, x11_legacy={"wayland_protocol", "toolkit_module", "xim"},
        electron_x11={"gtk_module", "xim"}, electron_wayland={"wayland_protocol", "gtk_module"},
        electron_chromium={"wayland_protocol", "gtk_module", "xim"}, java={"xim"},
        unknown={"wayland_protocol", "gtk_module", "qt_module", "sdl_module", "xim"},
    }
    return sai.json.array(paths[toolkit] or paths.unknown)
end

return M
