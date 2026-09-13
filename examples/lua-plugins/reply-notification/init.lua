local policy = require("policy")

sai.on("reply_end", policy.plan)

sai.register_command({
    name="preview",
    description="Preview notification data without displaying a notification or playing sound.",
    access="read_only",
    --- 【答复通知】【策略预览】通过正式命令入口检查通知内容，不进行投递
    --- @param arguments string 包含 surface、status、locale 的 JSON 文本
    --- @return table 通知数据列表
    execute=function(arguments)
        local event = sai.json.decode(arguments)
        assert(type(event) == "table", "notification event must be an object")
        local notification = policy.plan(event)
        return {notifications=sai.json.array(notification and {notification} or {})}
    end,
})
