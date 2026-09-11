local hexagrams = require("data.zhouyi")
local cards = require("data.tarot")
local lots = require("data.fortune")
local orientations = {"正位", "逆位"}
local M = {}

--- 【玄学工具】【均匀抽取】从非空数组的全部元素中随机选择一项
--- @param items table 卦名、牌名、方位或签文数组
--- @return any 选中的完整元素
local function choose(items)
    return items[math.random(1, #items)]
end

--- 【玄学工具】【周易起卦】随机返回一个完整卦名，无参数
--- @return string 六十四卦之一
function M.zhouyi()
    return choose(hexagrams)
end

--- 【玄学工具】【塔罗抽牌】独立抽取牌名和正逆位置，无参数
--- @return string 使用全角括号连接的牌名与位置
function M.tarot()
    local card = choose(cards)
    local orientation = choose(orientations)
    return card .. "（" .. orientation .. "）"
end

--- 【玄学工具】【吉凶抽签】随机返回原等级和完整签文，无参数
--- @return string 使用全角冒号连接的等级与含义
function M.fortune()
    local lot = choose(lots)
    return lot[1] .. "：" .. lot[2]
end

return M
