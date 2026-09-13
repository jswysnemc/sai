local aliases = {
    ["美元"]="USD", ["美金"]="USD", ["人民币"]="CNY", ["元"]="CNY",
    ["日元"]="JPY", ["欧元"]="EUR", ["英镑"]="GBP", ["港币"]="HKD",
    ["台币"]="TWD", ["新台币"]="TWD", ["韩元"]="KRW",
}
local M = {}

--- 【汇率查询】【币种名称】保留中文别名和 Unicode 大写转换规则
--- @param value string 用户提供的币种
--- @return string 标准币种代码或去除首尾空白后的大写原值
function M.code(value)
    local code = sai.text.upper(sai.text.trim(value))
    return aliases[code] or code
end

return M
