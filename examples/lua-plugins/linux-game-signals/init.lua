local gather = require("gather")

sai.register_tool({
    name = "gather_linux_game_compatibility_signals",
    description = "Gather Steam, ProtonDB, Can I Play on Linux, and AreWeAntiCheatYet compatibility signals for one game. / 收集单个游戏在 Steam、ProtonDB、Can I Play on Linux、AreWeAntiCheatYet 上的兼容性信号。",
    access = "read_only",
    parameters = {
        type = "object",
        properties = {
            game = { type = "string", description = "Game title. / 游戏名称。" },
            issue = { type = "string", description = "Optional issue such as crash, multiplayer, anti-cheat, performance, mods. / 可选关注点，例如崩溃、多人、反作弊、性能、Mod。" },
        },
        required = sai.json.array({ "game" }),
        additionalProperties = false,
    },
    execute = gather.run,
})
