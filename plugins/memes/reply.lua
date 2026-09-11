local values = require("values")
local paths = require("paths")
local search = require("search")
local library = require("library")
local settings = require("settings")
local state = require("reply_state")
local decision = require("reply_decision")
local M = {}

--- 【表情自动发送】【只读准备】读取历史并选择候选，投递资料交给宿主按轮次绑定
--- @param input string 当前用户消息
--- @param ctx table 含只读权限和后续投递许可的上下文
--- @param config table 设置
--- @return table 上下文、当前轮提醒及可选投递对象
function M.prepare(input, ctx, config)
    local result = {context=state.reminder(state.load(config))}
    if not ctx.reply_can_deliver or not config.auto_send_enabled or sai.text.trim(input) == ""
        or config.auto_send_probability <= 0 then return result end
    if values.float32(math.random()) > math.min(1, math.max(0, config.auto_send_probability)) then return result end
    local name = paths.selected({}, config)
    local candidates = search.rank(config, name, input, {}, 12)
    if #candidates == 0 then candidates = search.rank(config, name, "", {}, 12) end
    if #candidates == 0 then return result end
    local choice = decision.choose(input, candidates)
    if not choice or not choice.send
        or choice.confidence < math.min(1, math.max(0, config.auto_send_min_confidence)) then return result end
    for _, candidate in ipairs(candidates) do
        local item = candidate.loaded.item
        if values.ids_match(item.id, choice.id) then
            local event = {library=name, id=item.id, name=item.name, description=item.description,
                usage=item.usage, reason=choice.reason, sent_at=sai.time.iso(sai.time.now())}
            result.reminder = "<system-reminder>\n本轮回复发送后，程序会自动发送一张表情包。你在回复文字时应该自然地知道这件事，让语气和表情一致，但不要直白说“我将发送表情包”。\n计划发送表情："
                .. values.display_name(event.name) .. "\n表情描述：" .. event.description
                .. "\n适用场景：" .. event.usage .. "\n选择原因：" .. event.reason .. "\n</system-reminder>"
            result.delivery = {event=event}
            return result
        end
    end
    return result
end

--- 【表情自动发送】【完成投递】重新查找当前仍启用的条目，显示成功后才记录最近发送
--- @param delivery table 宿主保存的原准备资料
--- @param ctx table 再次确认的可信写入权限
--- @param config table 当前实例设置
--- @return table 更新后的插件上下文
function M.complete(delivery, ctx, config)
    assert(ctx.allow_writes, "read-only callback cannot deliver memes")
    local event = delivery.event
    local loaded = assert(library.find(config, event.library, event.id), "meme not found: " .. event.id)
    sai.terminal.display_image(library.display_path(loaded), settings.configured(config, sai.terminal.size()))
    state.save(config, event)
    return {context=state.reminder(event)}
end

--- 【表情自动发送】【策略注册】插件设置固定在实例中，不向宿主暴露表情业务规则
--- @param config table 本实例设置
--- @return nil 注册完成
function M.register(config)
    sai.register_reply_policy({
        prepare=function(input, ctx) return M.prepare(input, ctx, config) end,
        complete=function(delivery, ctx) return M.complete(delivery, ctx, config) end,
    })
end

return M
