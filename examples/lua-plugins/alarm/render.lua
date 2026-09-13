local M = {}

--- 【闹钟】【兼容字段】把通用调度记录转换成旧闹钟公开字段
--- @param task table 调度记录
--- @param payload table 提醒参数
--- @return table 闹钟字段
function M.fields(task, payload)
    return {
        id=payload.legacy_id or task.id, time=payload.time, label=payload.label,
        audio_file=payload.audio_file, due_at=task.due_at,
        due_at_local=sai.time.local_format("%Y-%m-%d %H:%M:%S", task.due_at), pid=task.pid,
    }
end

--- 【闹钟】【创建结果】保持旧创建接口的 JSON 字段
--- @param task table 调度结果
--- @param payload table 提醒参数
--- @return string JSON 文本
function M.created(task, payload)
    local value = M.fields(task, payload)
    value.ok = true
    return sai.json.encode(value)
end

--- 【闹钟】【取消结果】返回实际取消是否接受，不把缺失任务报告为成功
--- @param id string 用户提交的闹钟标识
--- @param removed boolean 取消结果
--- @return string JSON 文本
function M.cancelled(id, removed)
    return sai.json.encode({ok=removed, id=id, removed=removed})
end

return M
