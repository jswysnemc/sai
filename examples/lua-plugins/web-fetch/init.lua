local arguments = require("arguments")
local request = require("request")
local output = require("output")

--- 【网页读取】【工具执行】组合参数归一、原始请求和有界正文输出
--- @param args table 公开工具参数
--- @param ctx table 本次调用上下文
--- @return string 转换及裁剪后的网页正文
local function execute(args, ctx)
    local options = arguments.parse(args, ctx)
    return output.render(request.fetch(options), options)
end

-- 1. 【网页读取】【注册契约】保留原工具名称、说明、参数和只读分类
sai.register_tool({
    name = "web_fetch",
    description = "Fetch a URL and return markdown, text, or html. Prefer this for opening a known URL. Does not search the web.",
    access = "read_only",
    parameters = {
        type = "object",
        properties = {
            url = {type = "string", description = "Fully-qualified http or https URL."},
            format = {type = "string", enum = sai.json.array({"markdown", "text", "html"}), description = "Output format. Defaults to markdown."},
            timeout = {type = "integer", description = "Timeout seconds, max 120."},
            max_chars = {type = "integer", description = "Maximum characters to return. Defaults to 24000, max 80000."},
        },
        required = sai.json.array({"url"}),
        additionalProperties = false,
    },
    execute = execute,
})
