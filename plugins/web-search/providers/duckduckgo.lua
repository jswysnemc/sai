local common = require("providers.common")
local user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36"

--- 【网页搜索】【HTML 实体】按原有顺序解码搜索页常用实体
--- @param value string HTML 片段
--- @return string 解码后的文本
local function unescape(value)
    return (value:gsub("&amp;", "&"):gsub("&quot;", '"'):gsub("&#x27;", "'")
        :gsub("&#39;", "'"):gsub("&lt;", "<"):gsub("&gt;", ">"))
end

--- 【网页搜索】【HTML 文本】保留宿主 HTML 解码并合并 Unicode 空白
--- @param value string 搜索结果 HTML 片段
--- @return string 紧凑纯文本
local function clean(value)
    return sai.text.collapse_whitespace(unescape(sai.text.html_to_text(value, 120)))
end

--- 【网页搜索】【摘要提取】读取结果后的首个摘要标签，保留原版截取位置
--- @param rest string 当前链接之后的 HTML
--- @return string 摘要文本，没有对应标签时为空
local function snippet(rest)
    local start = rest:find("result__snippet", 1, true)
    if not start then return "" end
    local opening = rest:find(">", start, true)
    if not opening then return "" end
    local closing = rest:find("</", opening + 1, true)
    if not closing then return "" end
    return clean(rest:sub(opening + 1, closing - 1))
end

--- 【网页搜索】【DuckDuckGo 解析】从 HTML 提取标题、地址和摘要并限制数量
--- @param html string 已解码网页
--- @param maximum integer 最大结果数
--- @return table 标题、地址和摘要组成的有序结果
local function parse(html, maximum)
    local results, rest = {}, html
    while true do
        local link = rest:find("result__a", 1, true)
        if not link then break end
        rest = rest:sub(link)
        local _, href_prefix = rest:find('href="', 1, true)
        if not href_prefix then break end
        local href_end = rest:find('"', href_prefix + 1, true)
        if not href_end then break end
        local url = unescape(rest:sub(href_prefix + 1, href_end - 1))
        local tag_end = rest:find(">", href_end, true)
        if not tag_end then break end
        local title_end = rest:find("</a>", tag_end + 1, true)
        if not title_end then break end
        local title = clean(rest:sub(tag_end + 1, title_end - 1))
        local summary = snippet(rest:sub(title_end))
        if title ~= "" and url ~= "" then
            results[#results + 1] = { title = title, url = url, snippet = summary }
        end
        if #results >= maximum then break end
        rest = rest:sub(title_end)
    end
    return results
end

--- 【网页搜索】【DuckDuckGo 查询】使用不需要密钥的原 HTML 回退地址
--- @param input table 归一化后的查询参数
--- @param config table 搜索设置
--- @return string 包含链接和摘要的 Markdown
local function search(input, config)
    local url = "https://html.duckduckgo.com/html/?q=" .. sai.text.url_encode(input.query)
    local html = common.request(url, "GET", { ["user-agent"] = user_agent }, nil, config)
    local results = parse(html, input.max_results)
    assert(#results > 0, "DuckDuckGo returned no parseable results")
    local lines = { "## Search results for: " .. input.query, "**Provider**: DuckDuckGo HTML fallback\n" }
    for index, item in ipairs(results) do
        lines[#lines + 1] = "### " .. index .. ". " .. item.title
        lines[#lines + 1] = "**URL**: " .. item.url
        if item.snippet ~= "" then lines[#lines + 1] = "**Snippet**: " .. item.snippet end
        lines[#lines + 1] = ""
    end
    return table.concat(lines, "\n")
end

return search
