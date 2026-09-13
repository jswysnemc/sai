local tasks = require("tasks")
local delivery = require("delivery")
assert(next(sai.config) == nil, "alarm has no settings; declare audio paths in the manifest and grants")

-- 1. 【闹钟】【工具注册】保留公开名称和独立读写声明
sai.register_tool({
    name="set_alarm",
    description="Set a local alarm or countdown. Accepts duration like 30s, 10m, 1h 30m, or a time like 14:30. The alarm runs in a background Sai process and uses Sai's embedded sound.",
    access="writes",
    parameters={
        type="object",
        properties={
            time={type="string", maxLength=256, description="Duration or clock time."},
            label={type="string", maxLength=4096, description="Optional alarm label."},
            audio_file={type="string", maxLength=8192, description="Optional local .wav or .mp3 audio file to play instead of Sai's built-in alarm sound."},
        },
        required=sai.json.array({"time"}), additionalProperties=false,
    },
    execute=tasks.set,
})
sai.register_tool({
    name="list_alarms", description="List currently scheduled or ringing local alarms.",
    access="read_only", parameters={type="object", properties={}, additionalProperties=false},
    execute=tasks.list,
})
sai.register_tool({
    name="cancel_alarm",
    description="Cancel a scheduled or ringing alarm by id. Use list_alarms first if the id is unknown.",
    access="writes",
    parameters={
        type="object", properties={id={type="string", maxLength=128, description="Alarm id from set_alarm or list_alarms."}},
        required=sai.json.array({"id"}), additionalProperties=false,
    },
    execute=tasks.cancel,
})

-- 2. 【闹钟】【后台入口】调度器只调用本插件已注册命令，投递逻辑继续由 Lua 维护
sai.register_command({
    name="deliver", description="Deliver one scheduled alarm sound.",
    access="writes", execute=delivery.run,
})
