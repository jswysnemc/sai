local arguments = require("arguments")
local M = {}
local statuses = {pending=true, in_progress=true, completed=true, cancelled=true}

--- 【待办状态】【值比较】忽略对象字段顺序，保持数组顺序与空数组类型
--- @param left any 待提交的 JSON 值
--- @param right any 已保存的 JSON 值
--- @return boolean 两个值是否相同
function M.equal(left, right)
    if type(left) ~= type(right) then return false end
    if type(left) ~= "table" then return left == right end
    if arguments.is_array(left) ~= arguments.is_array(right) then return false end
    for key, value in pairs(left) do
        if not M.equal(value, right[key]) then return false end
    end
    for key in pairs(right) do
        if left[key] == nil then return false end
    end
    return true
end

--- 【待办状态】【对象校验】禁止数组和 null 冒充状态对象
--- @param value any 原始 JSON 值
--- @return table 合法对象
local function object(value)
    assert(type(value) == "table" and not arguments.is_array(value), "todo state requires JSON objects")
    return value
end

--- 【待办状态】【条目校验】保留旧字符串原值，读写时丢弃原 serde 同样忽略的额外字段
--- @param items any 原始条目数组
--- @return table 经过结构检查的独立条目数组
local function checked_items(items)
    assert(arguments.is_array(items), "todo state items must be an array")
    local result, identifiers = sai.json.array(), {}
    for _, item in ipairs(items) do
        object(item)
        local next_item = {}
        for _, field in ipairs({"id", "text", "status", "created_at", "updated_at"}) do
            assert(type(item[field]) == "string", "invalid todo item field: " .. field)
            next_item[field] = item[field]
        end
        assert(statuses[item.status], "invalid todo item status")
        assert(not identifiers[item.id], "duplicate todo item id")
        identifiers[item.id] = true
        result[#result + 1] = next_item
    end
    return result
end

--- 【待办状态】【完整记录】活动清单与归档共用一个比较交换记录
--- @param value any 当前值或旧会话种子，null 表示尚无计划
--- @return table 可变业务副本
function M.decode(value)
    if value == nil or value == sai.json.null then
        return {version=1, items=sai.json.array(), history=sai.json.array()}
    end
    object(value)
    assert(value.version == 0 or value.version == 1, "unsupported todo state version")
    assert(arguments.is_array(value.history), "todo history must be an array")
    local result = {version=1, items=checked_items(value.items), history=sai.json.array()}
    for _, batch in ipairs(value.history) do
        object(batch)
        assert(type(batch.archived_at) == "string", "invalid todo archive time")
        result.history[#result.history + 1] = {archived_at=batch.archived_at, items=checked_items(batch.items)}
    end
    return result
end

return M
