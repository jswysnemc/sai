local M = {}
local fields = {
    add={path="string", name="string", format="string"}, list={format="string"},
    search={query="string", limit="integer"}, find={query="string", limit="integer"},
    read={file="string", start="integer", lines="integer"}, remove={file="string"},
    reindex={}, stats={}, ["embed-reindex"]={quiet="boolean", background="boolean", ticket="string"},
}

--- 【知识库命令参数】【无符号整数】命令支持原生整数及完整十进制文字，拒绝浮点近似
--- @param value any 输入值
--- @param name string 字段名
--- @return string|nil 标准十进制值，可选 null 返回 nil
local function unsigned(value, name)
    if value == sai.json.null then return nil end
    if math.type(value) == "integer" and value >= 0 then value = tostring(value) end
    assert(type(value) == "string" and value:match("^%d+$"), name .. " must be an unsigned integer or decimal string")
    value = value:gsub("^0+", "")
    if value == "" then value = "0" end
    assert(#value < 20 or (#value == 20 and value <= "18446744073709551615"), name .. " exceeds unsigned integer range")
    return value
end

--- 【知识库命令参数】【完整校验】只接受当前命令定义的字段，错误在进入文件操作前返回
--- @param name string 注册命令名
--- @param text string JSON 对象，空文字等同空对象
--- @return table 已校验且整数规范化的参数
function M.parse(name, text)
    local args = sai.text.trim(text) == "" and {} or sai.json.decode(text)
    assert(type(args) == "table" and getmetatable(args) ~= getmetatable(sai.json.array()), "knowledge base command arguments must be a JSON object")
    for key, value in pairs(args) do
        local kind = fields[name][key]
        assert(kind, "unknown knowledge base command argument: " .. tostring(key))
        if kind == "integer" then args[key] = unsigned(value, key)
        else assert(type(value) == kind, key .. " must be a " .. kind) end
    end
    assert(args.format == nil or args.format == "json" or args.format == "text", "format must be json or text")
    if args.ticket then
        assert(#args.ticket <= 19 and args.ticket:match("^[1-9]%d*$"), "ticket must be a positive queue sequence")
    end
    return args
end

return M
