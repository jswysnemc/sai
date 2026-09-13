local clock = require("time")
local audio = require("audio")
local payloads = require("payload")
local render = require("render")
local M = {}

--- 【闹钟】【分页记录】读取本插件的原生任务和旧版本兼容记录
--- @return table 全部有界记录
local function records()
    local result = {}
    for offset = 0, 256, 16 do
        local page = sai.scheduler.list({offset=offset, limit=16})
        for _, task in ipairs(page) do result[#result + 1] = task end
        assert(#result <= 256, "alarm task listing exceeded its bound")
        if #page < 16 then return result end
    end
    error("alarm task listing exceeded its bound")
end

--- 【闹钟】【创建】校验时间与音频，然后发布唯一的后台命令
--- @param args table 时间、标签和可选声音路径
--- @param ctx table 可信调用上下文
--- @return string 兼容创建结果
function M.set(args, ctx)
    assert(ctx.allow_writes, "read-only callback cannot set alarms")
    -- 1. 【闹钟】【业务校验】先完成纯参数检查与授权文件检查，再创建任务
    local time = sai.text.trim(args.time)
    local due = clock.due_at(time, sai.time.now())
    local label = payloads.label(args.label)
    local payload = {time=time, label=label, audio_file=audio.resolve(args.audio_file)}
    -- 2. 【闹钟】【持久发布】只保存业务参数，执行进程和持久状态由通用调度器维护
    local task = sai.scheduler.schedule({due_at=due, command="deliver", arguments=sai.json.encode(payload)})
    return render.created(task, payload)
end

--- 【闹钟】【活动查询】保留已设定和正在响铃的记录，终态与已接受取消的任务不再展示
--- @return string 兼容列表结果
function M.list()
    local alarms = sai.json.array()
    for _, task in ipairs(records()) do
        if task.command == "deliver" and (task.status == "scheduled" or task.status == "running") then
            local value = render.fields(task, payloads.decode(task.arguments))
            value.status = task.status == "running" and "ringing" or "scheduled"
            alarms[#alarms + 1] = value
        end
    end
    return sai.json.encode({ok=true, alarms=alarms})
end

--- 【闹钟】【标识查找】新任务直接查询，旧公开标识通过有界兼容记录映射
--- @param id string 闹钟标识
--- @return table|nil 对应活动记录
local function lookup(id)
    if #id == 36 and id:match("^job%-%x+$") and id == id:lower() then
        local task = sai.scheduler.get(id)
        if task ~= sai.json.null then return task end
        return nil
    end
    for _, task in ipairs(records()) do
        if task.command == "deliver" and payloads.decode(task.arguments).legacy_id == id then return task end
    end
end

--- 【闹钟】【取消】以任务身份请求取消，不接触持久 PID 或宿主信号
--- @param args table 含闹钟 id
--- @param ctx table 可信写入上下文
--- @return string 兼容取消结果
function M.cancel(args, ctx)
    assert(ctx.allow_writes, "read-only callback cannot cancel alarms")
    local id = sai.text.trim(args.id)
    assert(id ~= "" and #id <= 128, "id is required and must not exceed 128 bytes")
    local task = lookup(id)
    local active = task and task.command == "deliver" and (task.status == "scheduled" or task.status == "running" or task.status == "cancelling")
    return render.cancelled(id, not not (active and sai.scheduler.cancel(task.id)))
end

return M
