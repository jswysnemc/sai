local search = require("search")

sai.register_tool({
    name = "web_search",
    description = "Search the web with the configured provider. Auto mode tries enabled providers in order and uses DuckDuckGo HTML as the final built-in fallback.",
    access = "read_only",
    parameters = {
        type = "object",
        properties = {
            query = { type = "string", description = "Search query." },
            max_results = { type = "integer", description = "Maximum results. Uses the configured default when omitted." },
            provider = {
                type = "string",
                enum = sai.json.array({ "auto", "tinyfish", "tavily", "firecrawl", "anysearch", "searxng", "duckduckgo", "script" }),
                description = "Search provider. Uses the configured default when omitted. script is a legacy alias for duckduckgo.",
            },
            location = { type = "string", description = "Optional country code for TinyFish geo-targeted results, such as US or GB." },
            language = { type = "string", description = "Optional language code for TinyFish result language, such as en or fr." },
        },
        required = sai.json.array({ "query" }),
        additionalProperties = false,
    },
    execute = search.run,
})
