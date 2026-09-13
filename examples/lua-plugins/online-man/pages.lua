local arch = "https://man.archlinux.org"
local man7 = "https://man7.org/linux/man-pages"
local encode = sai.text.url_encode
local pages = {}

--- 【在线手册】【参数】取得非空文本参数
--- @param args table 工具参数
--- @param key string 参数名称
--- @return string 去除首尾空白的内容
local function required(args, key)
    local value = (args[key] or ""):match("^%s*(.-)%s*$")
    assert(value ~= "", key .. " is required")
    return value
end

--- 【在线手册】【请求】取得成功响应的文本，失败交给调用方选择备用来源
--- @param url string 已构造的在线手册地址
--- @return string 手册或搜索结果正文
local function fetch_text(url)
    local response = sai.http.request({ url = url, max_bytes = 4194304 })
    assert(response.status >= 200 and response.status < 300, "man page HTTP status " .. response.status)
    return response.text
end

--- 【在线手册】【搜索】搜索手册名称并返回去重后的页面链接
--- @param args table 包含 query、section、language 和 limit
--- @return string 可阅读的搜索结果
function pages.search(args)
    local query = required(args, "query")
    local language = (args.language or "en"):match("^%s*(.-)%s*$")
    local section = (args.section or ""):match("^%s*(.-)%s*$")
    local limit = math.min(50, args.limit and args.limit >= 0 and args.limit or 10)
    local url = arch .. "/search?q=" .. encode(query) .. "&lang=" .. encode(language)
    if section ~= "" then url = url .. "&section=" .. encode(section) end
    local html = fetch_text(url)
    local results, seen = {}, {}
    for path in html:gmatch("href=['\"](/man/[^'\"]+)['\"]") do
        if #results >= limit then break end
        local filename = path:match("([^/]+)$")
        local name = filename and filename:match("^(.+)%.[^%.]+%.[^%.]+$")
        if name and not seen[name] then
            results[#results + 1] = "- " .. name .. ": " .. arch .. path
            seen[name] = true
        end
    end
    if #results == 0 then return "No man page search results for " .. query end
    return table.concat(results, "\n")
end

--- 【在线手册】【阅读】依次尝试指定章节，并在需要时使用 man7 来源
--- @param args table 包含 name、section、source、language 和 max_chars
--- @return string 带来源链接的手册文本
function pages.get_page(args)
    local name = required(args, "name")
    local section = (args.section or ""):match("^%s*(.-)%s*$")
    local source = args.source or "auto"
    local language = args.language or "en"
    local max_chars = math.max(2000, math.min(100000, args.max_chars and args.max_chars >= 0 and args.max_chars or 16000))
    local sections = section ~= "" and { section } or { "1", "8", "5", "7", "2", "3", "4", "6" }
    -- 1. 优先读取 Arch 的纯文本手册
    if source == "auto" or source == "arch" then
        for _, current in ipairs(sections) do
            local url = arch .. "/man/" .. encode(name) .. "." .. encode(current) .. "." .. encode(language) .. ".txt"
            local ok, text = pcall(fetch_text, url)
            if ok then return sai.text.clip("Source: " .. url .. "\n\n" .. text, max_chars) end
        end
    end
    -- 2. man7 返回 HTML，由通用宿主能力转为文本
    if source == "auto" or source == "man7" then
        for _, current in ipairs(sections) do
            local url = man7 .. "/man" .. encode(current:sub(1, 1)) .. "/" .. encode(name) .. "." .. encode(current) .. ".html"
            local ok, html = pcall(fetch_text, url)
            if ok then return sai.text.clip("Source: " .. url .. "\n\n" .. sai.text.html_to_text(html, 120), max_chars) end
        end
    end
    error("man page not found: " .. name)
end

return pages
