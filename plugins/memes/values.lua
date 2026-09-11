local M = {}
local array_type = getmetatable(sai.json.array())

--- 【表情库】【字符串取值】仅接受字符串，不把 null、数字或布尔值转成文字
--- @param value any 待检查字段
--- @return string 原字符串或空字符串
function M.string(value) return type(value) == "string" and value or "" end

--- 【表情库】【数组类型】区分 JSON 数组和对象，空数组同样保留其类型
--- @param value any JSON 解码结果
--- @return boolean 是否为数组
function M.is_array(value) return type(value) == "table" and getmetatable(value) == array_type end

--- 【表情库】【字符串数组】沿用原版过滤规则，忽略非字符串及空白项
--- @param value any 标签输入
--- @return table 带 JSON 数组类型的有效字符串列表
function M.strings(value)
    local result = sai.json.array()
    if not M.is_array(value) then return result end
    for _, item in ipairs(value) do
        local text = sai.text.trim(M.string(item))
        if text ~= "" then result[#result + 1] = text end
    end
    return result
end

--- 【表情库】【必填字符串】执行 Unicode 空白清理，不接受隐式类型转换
--- @param args table 工具输入
--- @param key string 字段名称
--- @return string 非空字段；缺失时报告原版错误
function M.required(args, key)
    local value = sai.text.trim(M.string(args[key]))
    assert(value ~= "", key .. " is required")
    return value
end

--- 【表情库】【ASCII 小写】只转换 ASCII 字母，保留非 ASCII 大小写
--- @param value string 输入文本
--- @return string 转换后的文本
function M.lower(value)
    return (value:gsub("[A-Z]", function(letter) return string.char(letter:byte() + 32) end))
end

--- 【表情库】【检索归一化】仅替换 ASCII 标点，不折叠空白
--- @param value string 待检索文字
--- @return string 与原版一致的文本
function M.normalize(value)
    return (M.lower(value):gsub("[!-/:-@%[%]\\%^_`{|}~]", " "))
end

--- 【表情库】【库名清理】逐个 Unicode 字符替换，避免一个汉字产生多个短横线
--- @param value string 原库名
--- @return string 安全单目录名称，空结果使用 default
function M.sanitize(value)
    local parts = {}
    for _, code in utf8.codes(value) do
        local char = utf8.char(code)
        parts[#parts + 1] = char:match("^[A-Za-z0-9_-]$") and M.lower(char) or "-"
    end
    local result = table.concat(parts):gsub("^%-+", ""):gsub("%-+$", "")
    return result ~= "" and result or "default"
end

--- 【表情库】【标识归一化】只去掉小写 sha256 前缀及两侧 Unicode 空白
--- @param value string 存储或请求标识
--- @return string 待匹配部分
function M.id_part(value) return (sai.text.trim(value):gsub("^sha256:", "")) end

--- 【表情库】【标识匹配】请求必须是存储标识的非空前缀，匹配具有方向
--- @param stored string 存储标识
--- @param requested string 请求标识
--- @return boolean 是否匹配
function M.ids_match(stored, requested)
    stored, requested = M.id_part(stored), M.id_part(requested)
    return requested ~= "" and stored:sub(1, #requested) == requested
end

--- 【表情库】【显示名称】按中文、英文、缺省名称选择，不修改原名称空白
--- @param name any 本地化名称
--- @return string 显示文字
function M.display_name(name)
    if type(name) == "table" then
        for _, key in ipairs({"zh", "en"}) do
            local text = M.string(name[key])
            if sai.text.trim(text) ~= "" then return text end
        end
    end
    return "未命名表情"
end

--- 【表情库】【JSON 截取】取第一个左括号到最后一个右括号，不修复非法 JSON
--- @param text string 模型正文
--- @return string|nil 候选 JSON 片段
function M.json_slice(text)
    local first = text:find("{", 1, true)
    local last = text:match(".*()}")
    if first and last and last >= first then return text:sub(first, last) end
end

--- 【表情库】【原始整数】使用宿主原始 JSON 查询，保留浮点表示和完整 u64 的差别
--- @param ctx table 可信上下文
--- @param key string 参数键
--- @param fallback integer 缺省值
--- @param maximum integer 最大值
--- @return integer 有界非负整数
function M.integer(ctx, key, fallback, maximum)
    local text = ctx.json_integer("/" .. key)
    if not text or text:sub(1, 1) == "-" then return fallback end
    if #text > #tostring(maximum) then return maximum end
    return math.min(tonumber(text), maximum)
end

--- 【表情库】【资料复制】复制小型 JSON 对象，避免在条件重试间复用可变引用
--- @param value any 可序列化值
--- @return any 独立 JSON 副本
function M.copy(value) return sai.json.decode(sai.json.encode(value)) end

--- 【表情库】【单精度数值】沿用旧概率配置与模型置信度的 f32 舍入边界
--- @param value number JSON 数值
--- @return number 单精度舍入后的数值
function M.float32(value) return (string.unpack("f", string.pack("f", value))) end

return M
