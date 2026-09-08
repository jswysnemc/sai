local topics = require("topics")
local excerpt = require("excerpt")
local response = require("response")

--- 【Fcitx Wiki】【查询】选择官方规则，并按需获取页面摘录
--- @param args table 问题、主题、语言和摘录开关
--- @return table 保留兼容字段的规则查询结果
local function query(args)
    local text = sai.text.trim(args.query or "")
    local topic = topics.select(sai.text.trim(args.topic or "auto"), text)
    local page = topics.page(topic)
    local page_excerpt = sai.json.null
    if args.include_page_excerpt ~= false then
        local ok, result = pcall(excerpt.fetch, page.url)
        page_excerpt = ok and result or ("[fetch failed: " .. tostring(result) .. "]")
    end
    return response.format(topic, text, page, args.language or "bilingual", page_excerpt)
end

sai.register_tool({
    name = "fcitx5_input_method_wiki_qurey",
    description = "Query official Fcitx 5 Wiki guidance for Linux input method diagnosis. Returns bilingual structured claims from a small official-page whitelist; use after check_issue for Fcitx/XIM/GTK/Qt/Wayland questions. This is not general web search.",
    parameters = {
        type = "object",
        properties = {
            query = { type = "string", description = "Natural language question, e.g. XWayland XIM, Electron input method, GTK_IM_MODULE, LC_CTYPE." },
            topic = { type = "string", enum = { "auto", "home", "for_users", "setup", "wayland", "environment_variables", "xim", "gtk", "qt", "electron_chromium", "locale" }, description = "Focused Fcitx Wiki topic. Defaults to auto." },
            language = { type = "string", enum = { "bilingual", "zh", "en" }, description = "Output language wrapper. Defaults to bilingual." },
            include_page_excerpt = { type = "boolean", description = "Fetch and include a clipped excerpt from the official wiki page. Defaults to true." },
        },
        required = sai.json.array(), additionalProperties = false,
    },
    access = "read_only",
    execute = query,
})
