local paths = require("paths")
local index = require("index")
local values = require("values")
local M = {}

--- 【表情自动发送】【历史读取】沿用旧记录格式，拒绝损坏字段进入模型上下文
--- @param config table 设置
--- @return table|nil 最近发送事件
function M.load(config)
    local state = index.read_json(paths.state(config))
    if state == nil then return nil end
    assert(type(state) == "table" and not values.is_array(state), "invalid meme auto-send state")
    local event = state.last
    if event == nil or event == sai.json.null then return nil end
    assert(type(event) == "table" and not values.is_array(event), "invalid meme auto-send event")
    for _, key in ipairs({"library", "id", "description", "usage", "reason", "sent_at"}) do
        assert(type(event[key]) == "string", "invalid meme auto-send field: " .. key)
    end
    assert(event.name ~= nil, "invalid meme auto-send name")
    return event
end

--- 【表情自动发送】【历史提醒】保留原版本最近发送提醒文字
--- @param event table|nil 最近发送事件
--- @return string|nil 模型上下文
function M.reminder(event)
    if not event then return nil end
    return "<system-reminder>\n上一轮回复文字发送后，程序自动补发了一张表情包。你需要自然地记得自己已经发过这张表情；如果用户提到“刚才那张/你发的表情”，按这个信息回答，不要说不知道。\n表情库："
        .. event.library .. "\n表情名：" .. values.display_name(event.name)
        .. "\n表情描述：" .. event.description .. "\n适用场景：" .. event.usage
        .. "\n发送原因：" .. event.reason .. "\n发送时间：" .. event.sent_at .. "\n</system-reminder>"
end

--- 【表情自动发送】【最近查询】保留 recent_meme 的原结果形状和空记录文案
--- @param config table 设置
--- @return table 公开查询结果
function M.recent(config)
    local last = M.load(config)
    if last then return {success=true, recent=last} end
    return {success=false, message="当前人格/表情库还没有自动发送过表情"}
end

--- 【表情自动发送】【记录发布】只在图片显示成功后条件更新最近事件
--- @param config table 设置
--- @param event table 已显示事件
--- @return nil 冲突持续发生时返回明确错误
function M.save(config, event)
    local path = paths.state(config)
    for _ = 1, index.attempts do
        local _, expected = index.read_json(path)
        if index.publish(path, {last=event}, expected) then return end
    end
    error("meme auto-send state changed concurrently; delivery was not recorded")
end

return M
