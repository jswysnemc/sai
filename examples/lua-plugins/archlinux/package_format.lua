local M = {}
local null = sai.json.null

--- 【Arch 查询】【字段兼容】将缺失字段转换为 JSON null，保留其他值
--- @param value any 原始字段
--- @return any 可序列化字段
local function nullable(value)
    if value == nil then return null end
    return value
end

--- 【Arch 查询】【依赖列表】保留数组并将单值包装成数组
--- @param value any 原接口字段
--- @return table 空数组、原数组或单项数组
local function as_list(value)
    if value == nil or value == null then return sai.json.array() end
    if type(value) == "table" and getmetatable(value) == getmetatable(sai.json.array()) then
        return value
    end
    return sai.json.array({ value })
end

--- 【Arch 查询】【时间格式】把整数 Unix 时间转换为 UTC ISO 字符串
--- @param value any 原接口时间戳
--- @return string|userdata 有效时间或 JSON null
local function timestamp(value)
    if math.type(value) ~= "integer" then return null end
    return sai.time.iso(value) or null
end

--- 【Arch 查询】【搜索结果】统一 AUR 软件包字段和缺省值
--- @param item table 原始 AUR 条目
--- @return table 兼容原工具的搜索结果
function M.search(item)
    item = type(item) == "table" and item or {}
    local name = type(item.Name) == "string" and item.Name or ""
    return {
        name = name,
        package_base = nullable(item.PackageBase),
        version = nullable(item.Version),
        description = nullable(item.Description),
        votes = nullable(item.NumVotes),
        popularity = nullable(item.Popularity),
        maintainer = nullable(item.Maintainer),
        out_of_date = item.OutOfDate ~= nil and item.OutOfDate ~= null,
        out_of_date_at = nullable(item.OutOfDate),
        last_modified = nullable(item.LastModified),
        last_modified_iso = timestamp(item.LastModified),
        upstream_url = nullable(item.URL),
        aur_url = name ~= "" and ("https://aur.archlinux.org/packages/" .. name) or null,
    }
end

--- 【Arch 查询】【包详情】在公共字段上补充提交时间和依赖信息
--- @param item table 原始 AUR 条目
--- @return table 兼容原工具的详情结果
function M.info(item)
    item = type(item) == "table" and item or {}
    local result = M.search(item)
    result.first_submitted = nullable(item.FirstSubmitted)
    result.first_submitted_iso = timestamp(item.FirstSubmitted)
    result.url_path = nullable(item.URLPath)
    local fields = {
        license = "License", keywords = "Keywords", depends = "Depends",
        make_depends = "MakeDepends", check_depends = "CheckDepends",
        opt_depends = "OptDepends", provides = "Provides", conflicts = "Conflicts",
    }
    for target, source in pairs(fields) do result[target] = as_list(item[source]) end
    return result
end

return M
