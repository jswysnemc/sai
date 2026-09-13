local M = {}
local home = { title = "Fcitx 5", path = "Fcitx_5" }
local setup = { title = "Setup Fcitx 5", path = "Setup_Fcitx_5" }
local wayland = { title = "Using Fcitx 5 on Wayland", path = "Using_Fcitx_5_on_Wayland" }
local environment = {
    title = "Input method related environment variables",
    path = "Input_method_related_environment_variables",
}
local pages = {
    home = home, for_users = home, setup = setup, wayland = wayland,
    environment_variables = environment, xim = environment, gtk = environment,
    qt = environment, electron_chromium = wayland, locale = environment,
}
local auto_topics = {
    { "xim", { "xim", "xmodifiers", "xwayland" } },
    { "electron_chromium", { "electron", "chromium", "wechat" } },
    { "gtk", { "gtk" } },
    { "qt", { "qt" } },
    { "locale", { "lc_ctype", "locale", "lang" } },
    { "wayland", { "wayland", "text-input", "ozone" } },
    { "setup", { "setup", "install", "配置" } },
}

--- 【Fcitx Wiki】【主题选择】使用显式主题或按原优先级匹配问题
--- @param topic string 指定主题或 auto
--- @param query string 已修剪的问题文本
--- @return string 白名单主题
function M.select(topic, query)
    if topic ~= "auto" and topic ~= "" then
        assert(pages[topic], "unsupported fcitx wiki topic: " .. topic)
        return topic
    end
    local lower = query:lower()
    for _, rule in ipairs(auto_topics) do
        for _, term in ipairs(rule[2]) do
            if lower:find(term, 1, true) then return rule[1] end
        end
    end
    return "for_users"
end

--- 【Fcitx Wiki】【来源页面】从固定白名单选择官方页面
--- @param topic string 已确认的主题
--- @return table 含标题和来源 URL
function M.page(topic)
    local page = pages[topic] or home
    return { title = page.title, url = "https://fcitx-im.org/wiki/Special:MyLanguage/" .. page.path }
end

return M
