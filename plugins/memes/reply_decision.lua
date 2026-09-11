local values = require("values")
local M = {}

--- 【表情自动发送】【候选判断】向当前模型提交一次无工具请求，只允许选择候选标识
--- @param message string 当前用户消息
--- @param candidates table 有界候选
--- @return table|nil 通过原版字段检查的模型决策
function M.choose(message, candidates)
    local catalog = sai.json.array()
    for _, candidate in ipairs(candidates) do
        local item = candidate.loaded.item
        catalog[#catalog + 1] = {id=item.id, local_score=candidate.score, name=item.name,
            description=item.description, usage=item.usage, avoid=item.avoid, tags=item.tags}
    end
    local stop = utf8.offset(message, 1001)
    if stop then message = message:sub(1, stop - 1) end
    local prompt = "你在 Sai 回复前决定本轮是否应该搭配一张表情包。概率只控制触发频率；这里需要判断候选表情和用户消息、上下文语气的相关程度。请根据用户消息的语气、场景、关系边界和候选表情的 usage/avoid 决定。严肃、道歉、群管理、技术排障、长篇解释、用户明显在求助时不要发表情。轻松闲聊、调侃、打招呼、夸奖、吐槽、玩梗、情绪回应时可以发。只能从候选表情里选。confidence 表示所选表情与本轮消息/上下文的相关程度，0.0 到 1.0。只返回严格 JSON：{\"send\": false, \"id\": \"\", \"confidence\": 0.0, \"reason\": \"\"}\n\n用户消息："
        .. message .. "\n\n候选表情：" .. sai.json.encode(catalog)
    local response = sai.model.complete({messages={
        {role="system", content="你是表情包发送决策器，只输出 JSON，不输出解释。"},
        {role="user", content=prompt},
    }, tools=sai.json.array()})
    local text = values.json_slice(response.content)
    if not text then return nil end
    local ok, decision = pcall(sai.json.decode, text)
    if not ok or type(decision) ~= "table" or values.is_array(decision) then return nil end
    local defaults = {send=false, id="", confidence=0, reason=""}
    for key, default in pairs(defaults) do
        if decision[key] == nil then decision[key] = default end
        if type(decision[key]) ~= type(default) then return nil end
    end
    decision.confidence = values.float32(decision.confidence)
    if math.abs(decision.confidence) == math.huge then return nil end
    return decision
end

return M
