local M = {}

--- 【诊断配置】【范围校验】显式非法配置不能静默替换为默认值
--- @param name string 配置名称
--- @param fallback integer 缺省值
--- @param minimum integer 下限
--- @param maximum integer 上限
--- @return integer 有效配置
local function integer(name, fallback, minimum, maximum)
    local value = sai.config[name]
    if value == nil then value = fallback end
    assert(type(value) == "number" and value % 1 == 0 and value >= minimum and value <= maximum,
        name .. " must be an integer between " .. minimum .. " and " .. maximum)
    return value
end

M.command_timeout_ms = integer("command_timeout_ms", 5000, 1, 120000)
M.max_stdout_chars = integer("max_stdout_chars", 8000, 0, 200000)
M.max_stderr_chars = integer("max_stderr_chars", 4000, 0, 200000)

return M
