local payloads = require("payload")
local M = {}

--- 【闹钟】【单次投递】使用宿主通知能力播放一次声音，失败交给调度器记录
--- @param arguments string 持久化的提醒参数
--- @param ctx table 可信写入上下文
--- @return table 完成通道
function M.run(arguments, ctx)
    assert(ctx.allow_writes, "read-only callback cannot deliver alarms")
    local payload = payloads.decode(arguments)
    local sound = {builtin="alarm"}
    if payload.audio_file ~= sai.json.null then sound = {path=payload.audio_file} end
    local body = payload.legacy_id and "" or payload.label
    return sai.notify.send({title="Sai alarm", body=body, desktop=false, sound=sound, timeout_ms=120000})
end

return M
