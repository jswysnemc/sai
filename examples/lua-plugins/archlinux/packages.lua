local http = require("http")
local format = require("package_format")
local M = {}

--- 【Arch 查询】【必填参数】读取并修剪字符串参数
--- @param args table 工具参数
--- @param key string 字段名
--- @return string 非空参数值
local function required(args, key)
    local value = sai.text.trim(args[key] or "")
    assert(value ~= "", key .. " is required")
    return value
end

--- 【Arch 查询】【AUR 搜索】按查询方式读取并限制返回数量
--- @param args table 含 query、limit 和 search_by
--- @return table 查询文本及归一化结果
function M.search(args)
    local query = required(args, "query")
    local limit = args.limit
    if limit == nil or limit < 0 then limit = 10 end
    limit = math.min(limit, 50)
    local url = "https://aur.archlinux.org/rpc/?v=5&type=search&by="
        .. sai.text.url_encode(args.search_by or "name-desc")
        .. "&arg=" .. sai.text.url_encode(query)
    local data = http.json(url)
    local results = sai.json.array()
    for index, item in ipairs(type(data.results) == "table" and data.results or {}) do
        if index > limit then break end
        results[#results + 1] = format.search(item)
    end
    return { success = true, query = query, results = results }
end

--- 【Arch 查询】【AUR 详情】查询最多五个包并保留缺失项
--- @param args table 含逗号或空格分隔的 package_name
--- @return table 请求、找到、缺失的包名及包详情
function M.info(args)
    local names = sai.json.array()
    for name in required(args, "package_name"):gmatch("[^, ]+") do
        name = sai.text.trim(name)
        if name ~= "" then names[#names + 1] = name end
        if #names == 5 then break end
    end
    assert(#names > 0, "package_name is required")
    local url = "https://aur.archlinux.org/rpc/?v=5&type=info"
    for _, name in ipairs(names) do url = url .. "&arg[]=" .. sai.text.url_encode(name) end
    local data = http.json(url)
    local found, missing, results = sai.json.array(), sai.json.array(), sai.json.array()
    local found_set = {}
    for _, item in ipairs(type(data.results) == "table" and data.results or {}) do
        if type(item) == "table" and type(item.Name) == "string" then
            found[#found + 1] = item.Name
            found_set[item.Name:lower()] = true
        end
        results[#results + 1] = format.info(item)
    end
    for _, name in ipairs(names) do
        if not found_set[name:lower()] then missing[#missing + 1] = name end
    end
    return { success = true, requested = names, found = found, missing = missing, results = results }
end

--- 【Arch 查询】【官方包】根据仓库参数选择搜索或精确详情
--- @param args table 含 package_name、repo、arch 和 mode
--- @return table 查询方式、来源和原始官方包数据
function M.official(args)
    local package = required(args, "package_name")
    local repo = sai.text.trim(args.repo or "")
    local arch = sai.text.trim(args.arch or "x86_64")
    local mode = args.mode or "auto"
    if mode == "auto" then mode = repo ~= "" and "detail" or "search" end
    local url
    if mode == "detail" then
        assert(repo ~= "", "repo is required for detail mode")
        url = "https://archlinux.org/packages/" .. sai.text.url_encode(repo)
            .. "/" .. sai.text.url_encode(arch) .. "/" .. sai.text.url_encode(package) .. "/json/"
    else
        url = "https://archlinux.org/packages/search/json/?name=" .. sai.text.url_encode(package)
    end
    return {
        success = true, mode = mode, package_name = package,
        repo = repo ~= "" and repo or sai.json.null, arch = arch, url = url,
        data = http.json(url, "sai-archlinux-official-package-query/0.1", 15000),
    }
end

return M
