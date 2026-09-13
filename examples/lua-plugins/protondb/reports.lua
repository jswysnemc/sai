local http = require("http")
local M = {}
local base = "https://www.protondb.com"

--- 【ProtonDB】【地址哈希】拼接按 64 位整数回绕的中间值
--- @param factor integer 乘数
--- @param value integer 原始计数或标识
--- @param timestamp integer 非零数据时间戳
--- @return string 与原实现一致的无符号十进制中间串
local function hash_part(factor, value, timestamp)
    return string.format("%up%u", value, factor * (value % timestamp))
end

--- 【ProtonDB】【评论标识】按前端的有符号 32 位规则计算文件标识
--- @param app_id integer Steam App ID
--- @param count integer 全站评论数
--- @param timestamp integer 数据时间戳
--- @return integer 评论文件标识
local function report_id(app_id, count, timestamp)
    local text = "p" .. hash_part(app_id, count, timestamp) .. "*vRT"
        .. hash_part(1, app_id, timestamp) .. "undefinedm"
    local hash = 0
    for index = 1, #text do hash = (hash * 31 + text:byte(index)) & 0xffffffff end
    if hash >= 0x80000000 then hash = hash - 0x100000000 end
    return math.abs(hash)
end

--- 【ProtonDB】【评论读取】根据当前计数版本获取游戏评论文件
--- @param app_id integer 已解析的 Steam App ID
--- @return table 评论总数和原始评论数组
function M.fetch(app_id)
    local counts = http.json(base .. "/data/counts.json")
    local count, timestamp = counts.reports, counts.timestamp
    assert(math.type(count) == "integer" and count > 0
        and math.type(timestamp) == "integer" and timestamp > 0, "invalid counts.json")
    return http.json(base .. "/data/reports/all-devices/app/"
        .. report_id(app_id, count, timestamp) .. ".json")
end

return M
