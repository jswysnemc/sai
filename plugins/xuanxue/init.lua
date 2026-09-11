local draw = require("draw")
local dice = require("dice")

-- 1. 【玄学工具】【抽取契约】保留三个无参数工具的名称和只读属性
sai.register_tool({
    name = "draw_zhouyi_hexagram",
    description = "Draw a Zhouyi hexagram.",
    access = "read_only",
    parameters = {type = "object", properties = {}, additionalProperties = false},
    execute = draw.zhouyi,
})

sai.register_tool({
    name = "draw_tarot_card",
    description = "Draw a tarot card.",
    access = "read_only",
    parameters = {type = "object", properties = {}, additionalProperties = false},
    execute = draw.tarot,
})

sai.register_tool({
    name = "draw_fortune_lot",
    description = "Draw a fortune result.",
    access = "read_only",
    parameters = {type = "object", properties = {}, additionalProperties = false},
    execute = draw.fortune,
})

-- 2. 【玄学工具】【骰子契约】沿用原整数 Schema，默认值与数量收窄由业务模块处理
sai.register_tool({
    name = "roll_dice",
    description = "Roll dice.",
    access = "read_only",
    parameters = {
        type = "object",
        properties = {
            count = {type = "integer", description = "Number of dice, default 1, max 100. / 骰子数量，默认 1，最多 100。"},
            sides = {type = "integer", description = "Sides per die, default 6, max 1000. / 每颗骰子的面数，默认 6，最多 1000。"},
            modifier = {type = "integer", description = "Optional total modifier. / 可选总和修正值。"},
        },
        additionalProperties = false,
    },
    execute = dice.roll,
})
