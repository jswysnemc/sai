-- 【Fcitx Wiki】【官方规则】按原工具内容迁移双语规则、适用范围和注意事项
return {
    home = {
        {
            zh = "Fcitx 5 是带插件扩展能力的输入法框架，主页的 For Users 区域链接到安装、设置、FAQ、Wayland 使用、技巧和升级页面。",
            en = "Fcitx 5 is an extensible input method framework; the home page's For Users section links to install, setup, FAQ, Wayland usage, tips, and upgrade pages.",
            applies_to = "orientation",
            caveat = "Use the home page to discover official user-facing pages, not to diagnose one app by itself.",
        },
    },
    for_users = {
        {
            zh = "For Users 不是单独的 User Guide 页面，而是 Fcitx 5 主页上的用户入口列表；诊断时优先跳到 Setup、Wayland、FAQ 或环境变量页面。",
            en = "For Users is a user-facing link section on the Fcitx 5 home page, not a standalone User Guide page; diagnostics should jump to Setup, Wayland, FAQ, or environment-variable pages.",
            applies_to = "navigation",
            caveat = "The literal User_Guide page is empty, so do not treat it as authoritative content.",
        },
    },
    setup = {
        {
            zh = "官方 Setup 页把 `XMODIFIERS=@im=fcitx`、`GTK_IM_MODULE=fcitx`、`QT_IM_MODULE=fcitx` 作为基础环境变量示例，但强调过渡期没有适合所有人的完美方案。",
            en = "The official Setup page lists `XMODIFIERS=@im=fcitx`, `GTK_IM_MODULE=fcitx`, and `QT_IM_MODULE=fcitx` as basic environment examples, while noting there is no perfect one-size-fits-all setup during the transition period.",
            applies_to = "setup",
            caveat = "Do not conclude a variable is required globally without considering toolkit, backend, compositor, and app packaging.",
        },
        {
            zh = "Setup 页提到 systemd `environment.d` 可配置会话环境，但变更通常需要重新登录或重启用户会话才生效。",
            en = "The Setup page mentions systemd `environment.d` for session environment, with changes generally requiring re-login or a restarted user session.",
            applies_to = "environment",
            caveat = "The target process environment is the evidence that matters, not just the current shell.",
        },
    },
    wayland = {
        {
            zh = "Wayland 页明确说 XWayland 下 X11 应用与普通 X11 几乎没有区别，因此仍需要 `XMODIFIERS=@im=fcitx`。",
            en = "The Wayland page says X11 applications under XWayland are nearly the same as normal X11, so `XMODIFIERS=@im=fcitx` is still needed.",
            applies_to = "xwayland",
            caveat = "This does not prove a specific app calls XIM; it makes XMODIFIERS relevant evidence for X11/XWayland paths.",
        },
        {
            zh = "现代 GTK3/GTK4 Wayland 应用可走 text-input-v3；理想设置通常不是全局强制 `GTK_IM_MODULE`。",
            en = "Modern GTK3/GTK4 Wayland applications can use text-input-v3; the ideal setup usually does not globally force `GTK_IM_MODULE`.",
            applies_to = "gtk-wayland",
            caveat = "Legacy, XWayland, compositor-specific, or per-app cases may still need module overrides.",
        },
        {
            zh = "text-input-v3 是 Wayland 原生输入法协议，对 GTK/Qt/SDL/Electron Wayland 原生应用都有效。可通过 `wayland-info` 命令检查 compositor 是否支持 `zwp_text_input_manager_v3` 接口，并检查 fcitx5 是否加载了 `libwaylandim.so`（Wayland 前端模块）。",
            en = "text-input-v3 is the Wayland native input method protocol, effective for GTK/Qt/SDL/Electron Wayland-native applications. Use `wayland-info` to check if the compositor advertises `zwp_text_input_manager_v3`, and check whether fcitx5 has loaded `libwaylandim.so` (Wayland frontend module).",
            applies_to = "text-input-v3",
            caveat = "Both conditions (compositor support + fcitx5 frontend) must be met; either alone is insufficient.",
        },
    },
    xim = {
        {
            zh = "`XMODIFIERS` 只影响 XIM，Fcitx 的常见值是 `@im=fcitx`。",
            en = "`XMODIFIERS` affects XIM only; Fcitx commonly uses `@im=fcitx`.",
            applies_to = "xim",
            caveat = "The variable is an activation request, not proof that the app uses XIM.",
        },
        {
            zh = "非 CJK locale 下如果不设置 `XMODIFIERS`，一些应用的 XIM 不会工作；同时 XIM 还要求 locale 有效，不能是 `C` 或 `POSIX`。",
            en = "In non-CJK locales, XIM may not work for some applications without `XMODIFIERS`; XIM also requires a valid locale and must not use `C` or `POSIX`.",
            applies_to = "locale",
            caveat = "Check `LANG`/`LC_CTYPE` in the target process and confirm the locale exists in `locale -a`.",
        },
        {
            zh = "Wayland 页说明 X11/XWayland 应用仍按 X11 路径处理，XWayland 本身不应被当作 XIM 失效的证据。",
            en = "The Wayland page says X11/XWayland applications still follow the X11 path; XWayland itself should not be treated as evidence that XIM cannot work.",
            applies_to = "xwayland",
            caveat = "App/toolkit support and actual behavior still need local evidence.",
        },
    },
    gtk = {
        {
            zh = "`GTK_IM_MODULE` 会覆盖 GTK 的自动输入法模块选择；如果指定模块找不到，GTK 会回退到自动选择。",
            en = "`GTK_IM_MODULE` overrides GTK's automatic input module selection; if the requested module is not found, GTK falls back to automatic selection.",
            applies_to = "gtk",
            caveat = "Inspect GTK immodule cache and loaded `im-*.so` before claiming a GTK module is active.",
        },
        {
            zh = "Fcitx 在 GTK immodule 中声明支持 `zh:ja:ko:*`，相关信息记录在 GTK immodule cache 中。",
            en = "Fcitx declares GTK immodule support for `zh:ja:ko:*`, and this information is recorded in GTK immodule cache files.",
            applies_to = "gtk-cache",
            caveat = "Locale-based selection must be checked against the actual cache in host/runtime/container.",
        },
    },
    qt = {
        {
            zh = "Qt 输入法模块不需要 GTK 那样的 cache；`QT_IM_MODULE` 会覆盖 Qt 默认选择。",
            en = "Qt input modules do not need GTK-style cache files; `QT_IM_MODULE` overrides Qt's default choice.",
            applies_to = "qt",
            caveat = "Bundled/proprietary Qt apps may lack fcitx/ibus platform input context plugins, so process/package evidence matters.",
        },
        {
            zh = "Qt 6.7 引入 `QT_IM_MODULES` 作为 fallback 顺序，例如 `wayland;fcitx;ibus`。",
            en = "Qt 6.7 introduced `QT_IM_MODULES` as a fallback order, for example `wayland;fcitx;ibus`.",
            applies_to = "qt6",
            caveat = "Qt 4/5 may still require `QT_IM_MODULE`, so do not replace all Qt rules with `QT_IM_MODULES`.",
        },
    },
    electron_chromium = {
        {
            zh = "Wayland 页把 XWayland 下的 Electron/Chromium 归入类似 GTK2 的传统路径；不要断言它们一定不看 XIM。",
            en = "The Wayland page treats Electron/Chromium under XWayland as a traditional path similar to GTK2; do not assert that they categorically ignore XIM.",
            applies_to = "electron-xwayland",
            caveat = "Specific Electron/CEF/AppImage builds may be patched or old; local runtime evidence and actual input behavior win.",
        },
        {
            zh = "原生 Wayland Chromium/Electron 路径需要 Ozone Wayland 和 Wayland IM 相关 flags；Electron 不支持 Chromium 的 GTK4 路径。",
            en = "Native Wayland Chromium/Electron paths need Ozone Wayland and Wayland IM flags; Electron does not support Chromium's GTK4 path.",
            applies_to = "electron-wayland",
            caveat = "Only apply this to confirmed native Wayland windows, not mixed `WAYLAND_DISPLAY` + `DISPLAY` environments.",
        },
    },
    locale = {
        {
            zh = "XIM 需要有效 locale；locale 必须出现在 `locale -a`，并且不能是 `C` 或 `POSIX`。",
            en = "XIM needs a valid locale; the locale must appear in `locale -a` and must not be `C` or `POSIX`.",
            applies_to = "locale",
            caveat = "Check the target process, not only the shell that launched Sai.",
        },
        {
            zh = "Wiki 提到某些 XIM 场景可以用 `LC_CTYPE=zh_CN.UTF-8` 作为 workaround，尤其是历史上 Emacs/Java 一类问题。",
            en = "The Wiki mentions `LC_CTYPE=zh_CN.UTF-8` as a workaround in some XIM cases, historically including Emacs/Java-like issues.",
            applies_to = "workaround",
            caveat = "Treat this as a testable workaround, not a universal root cause.",
        },
    },
}
