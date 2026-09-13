local arguments = require("arguments")
local transitions = require("transitions")
local storage = require("state")
local M = {}

--- 【会话待办】【批量创建】保留原毫秒、批次序号和 u16 随机后缀格式
--- @param texts table 已校验的非空文字
--- @return table 新条目数组
local function create(texts)
    local now = sai.time.utc_now()
    local items = sai.json.array()
    for offset, text in ipairs(texts) do
        items[#items + 1] = {
            id="todo_" .. now.unix_ms .. "_" .. (offset - 1) .. "_" .. math.random(0, 65535),
            text=text, status="pending", created_at=now.rfc3339, updated_at=now.rfc3339,
        }
    end
    return items
end

--- 【会话待办】【动作执行】写入权限来自宿主，失败前不发布活动清单或历史
--- @param args table 原工具参数
--- @param ctx table 可信调用上下文
--- @return table 原公开 JSON 结果
function M.execute(args, ctx)
    assert(ctx.allow_writes, "todo requires a writable invocation")
    local action, index = args.action, arguments.index(ctx)
    local created = action == "add" and create(arguments.texts(args)) or nil
    local target = arguments.optional(args.id)
    local next_status = type(args.status) == "string" and transitions.parse(args.status) or nil
    local state, changed = storage.change(function(state, now)
        -- 1. 【会话待办】【批量插入】缺省或超界追加，零序号沿用原规则插入首位
        if action == "add" then
            local position = #state.items + 1
            if index and not arguments.exceeds(index, #state.items) then position = math.max(1, tonumber(index)) end
            local identifiers = {}
            for _, item in ipairs(state.items) do identifiers[item.id] = true end
            for _, item in ipairs(created) do
                assert(not identifiers[item.id], "todo id collision; retry the operation")
                identifiers[item.id] = true
                table.insert(state.items, position, item)
                position = position + 1
            end
            return created
        end
        if action == "list" then return nil end
        assert(action == "update" or action == "remove", "unsupported todo action: " .. tostring(action or ""))
        -- 2. 【会话待办】【目标绑定】首次按序号找到的条目固定为 id，冲突重试不能误改其他条目
        local position = arguments.locate(state.items, target, index)
        target = state.items[position].id
        local item = state.items[position]
        if action == "remove" then
            table.remove(state.items, position)
            return sai.json.array({item})
        end
        -- 3. 【会话待办】【更新规则】先验证状态顺序，再改写文本和时间
        local has_text = type(args.text) == "string"
        assert(has_text or next_status, "todo update requires text or status")
        if next_status then transitions.validate(state.items, position, next_status) end
        if has_text then item.text = arguments.required(args.text, "todo text is required") end
        if next_status then item.status = next_status end
        item.updated_at = now
        return sai.json.array({item})
    end)
    if action == "list" then return {ok=true, items=state.items} end
    return {ok=true, changed=changed, items=state.items}
end

return M
