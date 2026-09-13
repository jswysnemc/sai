local definition = require("definition")
local actions = require("actions")
local state = require("state")
for key in pairs(sai.config) do assert(key == "language", "unknown todo setting: " .. tostring(key)) end

assert(sai.config.language == nil or sai.config.language == "en" or sai.config.language == "zh", "invalid todo language")
definition.access = "writes"
definition.execute = actions.execute
sai.register_tool(definition)
sai.register_command({name="snapshot", description="Read the session plan and archived history.", access="read_only", execute=state.snapshot})
sai.register_command({name="import", description="Import an explicit legacy snapshot into an empty session plan.", access="writes", execute=require("import").run})
sai.register_reply_policy(require("reminder"))
