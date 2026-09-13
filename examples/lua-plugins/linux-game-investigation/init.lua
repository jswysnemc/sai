local investigate = require("investigate")

sai.register_tool({
    name = "linux_game_compatibility",
    description = "Run the Linux game compatibility investigation sub-agent and return its final report. / 运行 Linux 游戏兼容性调查子代理并返回最终报告。",
    parameters = {
        type = "object",
        properties = {
            game = { type = "string", description = "Game title. / 游戏名称。" },
            issue = { type = "string", description = "Optional issue such as crash, multiplayer, anti-cheat, performance, mods. / 可选关注点，例如崩溃、多人、反作弊、性能、Mod。" },
        },
        required = sai.json.array({ "game" }),
        additionalProperties = false,
    },
    access = "read_only",
    execute = investigate.run,
})
