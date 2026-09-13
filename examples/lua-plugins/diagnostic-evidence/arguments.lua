local text = require("text")
local M = {}

--- 【诊断参数】【可选文本】去除空白并按原版字符数上限保留内容
--- @param args table 调用参数
--- @param name string 参数名称
--- @param limit integer 字符上限
--- @return string|nil 非空文本
local function optional(args, name, limit)
    if type(args[name]) ~= "string" then return nil end
    local value = sai.text.trim(args[name])
    return value ~= "" and text.prefix(value, limit) or nil
end

--- 【诊断参数】【领域推断】按原版顺序解释自然语言和可选目标
--- @param query string|nil 问题
--- @param target string|nil 目标
--- @return string 采集领域
function M.infer_area(query, target)
    local original = query or ""
    local lower = original:lower()
    --- 【诊断参数】【关键字组合】分别匹配原中文文本和小写英文文本
    --- @param chinese table 中文关键字
    --- @param english table 英文关键字
    --- @return boolean 任一组是否匹配
    local function matches(chinese, english)
        return text.contains_any(original, chinese) or text.contains_any(lower, english)
    end
    if matches({"输入法", "打不了中文", "候选框", "拼音", "fcitx", "ibus"}, {"ime", "input method", "fcitx", "ibus"}) then
        return "input_method"
    elseif matches({"没声音", "声音", "麦克风", "耳机"}, {"audio", "sound", "pipewire", "wireplumber"}) then
        return "audio"
    elseif matches({"屏幕分享", "黑屏", "截图", "录屏", "显示器"}, {"display", "screen", "wayland", "xwayland", "portal"}) then
        return "display"
    elseif matches({"更新", "安装包", "依赖", "包管理"}, {"pacman", "yay", "paru", "aur", "apt", "dnf", "brew"}) then
        return "package"
    elseif matches({"显卡", "驱动", "独显", "核显"}, {"gpu", "nvidia", "amd", "mesa", "vulkan"}) then
        return "gpu"
    elseif matches({"网络", "联网", "断网", "网卡", "wifi"}, {"network", "internet", "wifi", "dns"}) then
        return "network"
    elseif matches({"磁盘", "硬盘", "空间", "挂载", "btrfs"}, {"disk", "storage", "mount", "filesystem"}) then
        return "storage"
    elseif target or matches({"打不开", "启动不了", "闪退", "崩溃", "报错"}, {"crash", "cannot start", "won't open", "not open"}) then
        return "app"
    end
    assert(sai.text.trim(original) ~= "", "area is auto but query is empty; provide query or structured area")
    return "system"
end

--- 【诊断参数】【目标推断】按兼容顺序识别常用应用名称
--- @param value string 原问题
--- @return string|nil 命令名称
function M.infer_target(value)
    local lower = value:lower()
    for _, entry in ipairs({
        {"opencode", "opencode"}, {"linuxqq", "qq"}, {"qq", "qq"}, {"微信", "wechat"},
        {"wechat", "wechat"}, {"steam", "steam"}, {"firefox", "firefox"}, {"chrome", "chrome"},
        {"chromium", "chromium"}, {"vscode", "code"}, {"code", "code"},
    }) do
        if lower:find(entry[1], 1, true) or value:find(entry[1], 1, true) then return entry[2] end
    end
end

--- 【诊断参数】【枚举校验】去空白后匹配明确支持的参数值
--- @param value string 原值
--- @param choices table 允许值集合
--- @param name string 错误中的参数名称
--- @return string 规范值
local function choice(value, choices, name)
    local normalized = sai.text.trim(value)
    assert(choices[normalized], "unsupported diagnostic " .. name .. ": " .. value)
    return normalized
end

--- 【诊断参数】【整数边界】保留原版无效值缺省及范围收窄规则
--- @param value any 原值
--- @param fallback integer 缺省值
--- @param maximum integer 上限
--- @return integer 受限正整数
local function bounded(value, fallback, maximum)
    if type(value) ~= "number" or value < 0 or value % 1 ~= 0 then value = fallback end
    return math.max(1, math.min(value, maximum))
end

--- 【诊断参数】【完整解析】保留原有字段、领域别名和自动目标推断行为
--- @param args table 调用参数
--- @return table 标准化参数，缺省可选文本保留 JSON null
function M.parse(args)
    local query, target, symptom = optional(args, "query", 500), optional(args, "target", 160), optional(args, "symptom", 200)
    local raw = args.area
    if raw == nil then raw = args.mode end
    raw = type(raw) == "string" and sai.text.trim(raw) or "auto"
    local area
    if raw == "auto" then
        area = M.infer_area(query, target)
        target = target or M.infer_target(query or "")
    else
        area = choice(raw, {system=true, app=true, input_method=true, display=true, audio=true,
            package=true, package_update=true, gpu=true, network=true, storage=true}, "area")
        if area == "package_update" then area = "package" end
    end
    local depth = choice(type(args.depth) == "string" and args.depth or "normal", {quick=true,normal=true,full=true}, "depth")
    local platform = choice(type(args.platform) == "string" and args.platform or "auto", {auto=true,linux=true,macos=true}, "platform")
    return {
        query=query or sai.json.null, area=area, target=target or sai.json.null, symptom=symptom or sai.json.null,
        depth=depth, platform=platform, recent_minutes=bounded(args.recent_minutes, 30, 1440),
        allow_launch_probe=args.allow_launch_probe == true, launch_timeout_seconds=bounded(args.launch_timeout_seconds, 3, 15),
    }
end

--- 【诊断参数】【平台选择】显式覆盖优先，自动模式只认可实际支持的平台
--- @param value string 参数中的平台选择
--- @return string linux、macos 或 unsupported
function M.platform(value)
    if value ~= "auto" then return value end
    local actual = sai.system.platform
    return (actual == "linux" or actual == "macos") and actual or "unsupported"
end

return M
