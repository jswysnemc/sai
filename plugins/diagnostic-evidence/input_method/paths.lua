local text = require("text")
local classify = require("input_method.classify")
local M = {}

--- 【输入法路径】【匹配模块】按原证据顺序查找第一个命中的动态库
--- @param modules table 模块列表
--- @param needles table 小写关键字
--- @return string|nil 匹配项
local function module_match(modules, needles)
    for _, module in ipairs(modules) do
        if text.contains_any(module:lower(), needles) then return module end
    end
end

--- 【输入法路径】【目标区域】LC_CTYPE 优先于 LANG，缺失时使用 C
--- @param locale table 区域设置
--- @return string 目标区域名称
local function target_locale(locale)
    if type(locale.target_lc_ctype) == "string" then return locale.target_lc_ctype end
    if type(locale.target_lang) == "string" then return locale.target_lang end
    return "C"
end

--- 【输入法路径】【缓存区域】沿用原版首个匹配模块的区域匹配解释
--- @param module_name string 模块关键字
--- @param cache table GTK 缓存
--- @param locale table 区域设置
--- @return string 匹配说明，未找到时为空
function M.immodule_locale(module_name, cache, locale)
    local target = target_locale(locale)
    local prefix = target:match("^[^._]*")
    for _, entry in ipairs(cache) do
        if entry.module_name:find(module_name, 1, true) then
            if entry.locales:find("*", 1, true) then return "immodule_cache: " .. entry.module_name .. " matches any locale (*)" end
            local matches = false
            for item in (entry.locales .. ":"):gmatch("(.-):") do
                if item == prefix or item == target then matches = true end
            end
            return "immodule_cache: " .. entry.module_name .. " locale '" .. entry.locales .. "' "
                .. (matches and "matches" or "does NOT match") .. " target '" .. target .. "'"
        end
    end
    return ""
end

--- 【输入法路径】【环境证据】记录非空的环境变量，保持原文本值
--- @param evidence table 证据数组
--- @param env table 目标环境
--- @param name string 变量名
--- @return nil
local function env_evidence(evidence, env, name)
    local value = env[name]
    if type(value) == "string" and sai.text.trim(value) ~= "" then evidence[#evidence + 1] = name .. "=" .. value end
end

--- 【输入法路径】【XIM 配置】检查以分号分隔的配置值
--- @param value string|nil 环境值
--- @return boolean 是否明确设置 fcitx
local function xim_environment(value)
    if type(value) ~= "string" then return false end
    for item in (value .. ";"):gmatch("(.-);") do if sai.text.trim(item) == "@im=fcitx" then return true end end
    return false
end

--- 【输入法路径】【单项检查】根据目标环境、动态库、协议和区域信息形成路径证据
--- @param name string 路径名
--- @param env table|nil 目标环境
--- @param loaded table 运行时动态库
--- @param available table 磁盘动态库
--- @param cache table GTK 缓存
--- @param wayland table Wayland 协议证据
--- @param locale table 区域设置
--- @return table 命名路径检查
function M.check(name, env, loaded, available, cache, wayland, locale)
    env = type(env) == "table" and env or {}
    local evidence, missing = sai.json.array(), sai.json.array()
    local status = "unknown"
    if name == "wayland_protocol" then
        local compositor = wayland.compositor_supports_text_input_v3
        local frontend = wayland.fcitx5_wayland_frontend_loaded
        if compositor then evidence[#evidence + 1] = "compositor supports zwp_text_input_manager_v3"
        else missing[#missing + 1] = "compositor text-input-v3 protocol support" end
        if frontend then evidence[#evidence + 1] = "fcitx5 loaded libwaylandim.so (wayland frontend)"
        else missing[#missing + 1] = "fcitx5 wayland frontend (libwaylandim.so)" end
        status = compositor and frontend and "confirmed" or ((compositor or frontend) and "configured" or "missing")
    elseif name == "gtk_module" or name == "toolkit_module" then
        if name == "gtk_module" then env_evidence(evidence, env, "GTK_IM_MODULE") end
        local runtime = module_match(loaded, {"im-fcitx", "im-wayland", "im-xim", "im-ibus"})
        local disk = module_match(available, {"im-fcitx", "im-wayland", "im-xim", "im-ibus"})
        if runtime then
            evidence[#evidence + 1] = "runtime_loaded_module=" .. runtime
            status = "confirmed"
        elseif disk then
            evidence[#evidence + 1] = "available_on_disk=" .. disk
            local match = M.immodule_locale("fcitx", cache, locale)
            if match ~= "" then evidence[#evidence + 1] = match end
            status = "configured"
        else
            missing[#missing + 1] = "GTK input module .so (neither loaded nor on disk)"
            status = "missing"
        end
    elseif name == "qt_module" then
        env_evidence(evidence, env, "QT_IM_MODULE")
        env_evidence(evidence, env, "QT_IM_MODULES")
        local runtime = module_match(loaded, {"platforminputcontext", "libfcitx", "libibus"})
        local disk = module_match(available, {"platforminputcontext", "fcitx"})
        if runtime then evidence[#evidence + 1] = "runtime_loaded_module=" .. runtime; status = "confirmed"
        elseif disk then evidence[#evidence + 1] = "available_on_disk=" .. disk; status = "configured"
        else missing[#missing + 1] = "Qt platforminputcontext .so evidence"; status = "missing" end
    elseif name == "sdl_module" then
        env_evidence(evidence, env, "SDL_IM_MODULE")
        local runtime = module_match(loaded, {"libfcitx", "libibus", "sdl"})
        if runtime then evidence[#evidence + 1] = "runtime_loaded_module=" .. runtime; status = "confirmed"
        else missing[#missing + 1] = "SDL input bridge .so evidence"; status = "missing" end
    elseif name == "xim" then
        env_evidence(evidence, env, "XMODIFIERS")
        local configured = xim_environment(env.XMODIFIERS)
        if not configured then missing[#missing + 1] = "XMODIFIERS=@im=fcitx not set in target env" end
        if not locale.locale_valid then
            missing[#missing + 1] = "locale '" .. target_locale(locale) .. "' is C/POSIX or not in locale -a; XIM may not activate"
        end
        local runtime = module_match(loaded, {"im-xim", "libx11", "libxim"})
        local disk = module_match(available, {"im-xim"})
        if runtime then
            evidence[#evidence + 1] = "runtime_loaded_module=" .. runtime
            status = configured and locale.locale_valid and "confirmed" or "configured"
        elseif disk then
            evidence[#evidence + 1] = "available_on_disk=" .. disk
            local match = M.immodule_locale("xim", cache, locale)
            if match ~= "" then evidence[#evidence + 1] = match end
            status = configured and locale.locale_valid and "configured" or "missing"
        else missing[#missing + 1] = "im-xim.so not found on disk"; status = "missing" end
    else
        missing[#missing + 1] = "unknown path name"
    end
    return {name=name, status=status, evidence=evidence, missing=missing}
end

local debug_names = {
    unknown="Unknown", gtk="Gtk", qt="Qt", sdl="Sdl", java="Java", x11_legacy="X11Legacy",
    electron_chromium="ElectronChromium", electron_x11="ElectronX11", electron_wayland="ElectronWayland",
    x11="X11", x_wayland="XWayland", wayland_native="WaylandNative",
}

--- 【输入法路径】【综合状态】汇总适配器与各输入路径，保留原版完整、部分和缺失状态
--- @param profile table 工具包、运行状态、环境与采集证据
--- @return table 路径数组和总体状态
function M.evaluate(profile)
    local missing = sai.json.array()
    if not profile.runtime_observed then missing[#missing + 1] = "runtime process evidence" end
    if profile.toolkit == "unknown" then missing[#missing + 1] = "app toolkit/framework evidence" end
    local paths = sai.json.array({{
        name="app_adapter", status=#missing == 0 and "confirmed" or "unknown", missing=missing,
        evidence=sai.json.array({"toolkit=" .. debug_names[profile.toolkit], "display_mode=" .. debug_names[profile.display_mode]}),
    }})
    local confirmed, incomplete = false, true
    for _, name in ipairs(classify.relevant_paths(profile.toolkit)) do
        local check = M.check(name, profile.target_env, profile.loaded_input_modules, profile.available_input_modules,
            profile.immodule_cache, profile.wayland_protocol, profile.locale_info)
        paths[#paths + 1] = check
        confirmed = confirmed or check.status == "confirmed"
        incomplete = incomplete and (check.status == "missing" or check.status == "unknown")
    end
    return {paths=paths, overall=confirmed and "path_evidence_complete" or (incomplete and "path_evidence_incomplete" or "path_evidence_partial")}
end

return M
