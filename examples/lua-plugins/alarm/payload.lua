local audio = require("audio")
local M = {}

--- 【闹钟】【提醒标签】保留原有默认值和空白整理，拒绝通知接口不能接收的控制字符
--- @param value string|nil 标签
--- @return string 已校验标签
function M.label(value)
    if value == nil then value = "Sai alarm" end
    assert(type(value) == "string", "alarm label must be a string")
    value = sai.text.trim(value)
    assert(#value <= 4096, "alarm label exceeds 4096 bytes")
    for _, code in utf8.codes(value) do
        assert(not ((code < 32 and code ~= 9 and code ~= 10 and code ~= 13) or (code >= 127 and code <= 159)), "alarm label contains invalid control characters")
    end
    return value
end

--- 【闹钟】【任务参数】读取宿主记录中的有界参数，旧标识仅用于兼容展示和查找
--- @param text string 命令参数 JSON
--- @return table 已校验参数
function M.decode(text)
    assert(type(text) == "string" and #text <= 16384, "invalid alarm payload")
    local value = sai.json.decode(text)
    assert(type(value) == "table", "alarm payload must be an object")
    for key in pairs(value) do
        assert(key == "time" or key == "label" or key == "audio_file" or key == "legacy_id", "unknown alarm payload field")
    end
    assert(type(value.time) == "string" and #value.time <= 256, "invalid alarm time text")
    if value.legacy_id ~= nil then
        assert(type(value.legacy_id) == "string" and #value.legacy_id > 0 and #value.legacy_id <= 128, "invalid legacy alarm id")
        assert(type(value.label) == "string" and #value.label <= 4096, "invalid legacy alarm label")
    else
        value.label = M.label(value.label)
    end
    if value.audio_file == nil then value.audio_file = sai.json.null end
    if value.audio_file ~= sai.json.null then audio.check(value.audio_file) end
    return value
end

return M
