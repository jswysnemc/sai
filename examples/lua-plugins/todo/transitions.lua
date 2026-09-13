local M = {}
local names = {pending="Pending", in_progress="InProgress", completed="Completed", cancelled="Cancelled"}

--- 【会话待办】【未完成判断】pending 和 in_progress 保留在活动计划中
--- @param status string 条目状态
--- @return boolean 是否尚未结束
function M.unfinished(status)
    return status == "pending" or status == "in_progress"
end

--- 【会话待办】【状态校验】保持原状态名称及两侧空白处理
--- @param status string 输入状态
--- @return string 校验后的状态
function M.parse(status)
    status = sai.text.trim(status)
    assert(names[status], "unsupported todo status: " .. status)
    return status
end

--- 【会话待办】【顺序推进】检查前置未完成项和至多一个进行中项，保留原错误顺序
--- @param items table 活动清单
--- @param position integer 目标的 1 起始位置
--- @param next_status string 目标状态
--- @return nil 合法时无返回值
function M.validate(items, position, next_status)
    if next_status == "in_progress" or next_status == "completed" then
        for index = 1, position - 1 do
            if M.unfinished(items[index].status) then
                error("todo item " .. position .. " cannot advance before earlier item " .. index
                    .. " (" .. names[items[index].status] .. ") is completed or cancelled; update index "
                    .. index .. " first")
            end
        end
    end
    if next_status == "in_progress" then
        for index, item in ipairs(items) do
            assert(index == position or item.status ~= "in_progress", "only one todo item can be in progress")
        end
    end
end

return M
