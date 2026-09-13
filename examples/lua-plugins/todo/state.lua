local schema = require("state_schema")
local transitions = require("transitions")
local M = {}
local KEY = "plan"

--- 【待办状态】【完成收敛】整批结束时追加一次历史并清空活动清单
--- @param state table 当前业务副本
--- @param now string 本次操作的 UTC 时间
--- @return boolean 是否发生归档
function M.settle(state, now)
    if #state.items == 0 then return false end
    for _, item in ipairs(state.items) do
        if transitions.unfinished(item.status) then return false end
    end
    state.history[#state.history + 1] = {archived_at=now, items=state.items}
    state.items = sai.json.array()
    return true
end

--- 【待办状态】【原子修改】重新读取并合并冲突，活动清单与历史在同一次提交中发布
--- @param change function 对独立业务副本的修改函数
--- @return table 提交后的状态
--- @return any 业务修改结果
function M.change(change)
    local now = sai.time.utc_now().rfc3339
    for _ = 1, 16 do
        local previous = sai.storage.get(KEY)
        local state = schema.decode(previous)
        M.settle(state, now)
        local result = change(state, now)
        M.settle(state, now)
        if schema.equal(state, previous) then return state, result end
        local encoded = sai.json.encode(state)
        assert(#encoded <= 262144, "todo state exceeds 256 KiB")
        if sai.storage.compare_exchange(KEY, previous, state) then return state, result end
    end
    error("todo state changed too often; retry the operation")
end

--- 【待办状态】【界面快照】沿用原 list 的完成归档语义，不执行增删改操作
--- @return table 活动清单与全部归档
function M.snapshot()
    local state = M.change(function() return nil end)
    return {items=state.items, history=state.history}
end

--- 【待办状态】【提醒读取】只检查当前条目，不在观察回调中导入或归档文件
--- @return boolean 是否存在未完成条目
function M.has_unfinished()
    local state = schema.decode(sai.storage.get(KEY))
    for _, item in ipairs(state.items) do
        if transitions.unfinished(item.status) then return true end
    end
    return false
end

return M
