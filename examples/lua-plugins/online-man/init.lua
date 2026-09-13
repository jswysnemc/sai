local pages = require("pages")

sai.register_tool({
    name = "online_man_search",
    description = "Search online Linux man pages using Arch manual pages.",
    parameters = {
        type = "object",
        properties = {
            query = { type = "string" }, section = { type = "string" },
            language = { type = "string" }, limit = { type = "integer" },
        },
        required = { "query" }, additionalProperties = false,
    },
    execute = pages.search,
})

sai.register_tool({
    name = "online_man_get_page",
    description = "Fetch an online Linux man page from Arch man pages or man7.org.",
    parameters = {
        type = "object",
        properties = {
            name = { type = "string" }, section = { type = "string" },
            source = { type = "string", enum = { "auto", "arch", "man7" } },
            language = { type = "string" },
            max_chars = { type = "integer", description = "Maximum returned characters. Use at least 8000 for normal reading; omit unless user asks for a short excerpt." },
        },
        required = { "name" }, additionalProperties = false,
    },
    execute = pages.get_page,
})
