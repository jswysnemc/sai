local M = {}

--- 【百科查询】【站点选择】保留中文默认站点及 uk、ja 两个可选站点
--- @param name string|nil 站点名称
--- @return table API、页面读取及展示地址
function M.select(name)
    local origin = name == "uk" and "https://moegirl.uk"
        or name == "ja" and "https://ja.moegirl.org" or "https://zh.moegirl.org.cn"
    return {api=origin, base=origin, page=origin .. "/"}
end

return M
