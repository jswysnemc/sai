local text = require("strings")
local M = {}

--- 【网页搜图】【筛选结果】创建完整的未请求或失败记录
--- @param status string 状态名称
--- @param info table|nil 宿主模型标识
--- @param failure string|nil 失败原因
--- @return table 不会静默丢弃图片的记录
function M.empty(status, info, failure)
    return {status=status, accepted=true, description="", reason="", provider_id=info and info.provider_id or "",
        model=info and info.model or "", error=failure or ""}
end

--- 【网页搜图】【筛选初始化】区分关闭、未配置与授权或配置错误
--- @param config table 包设置
--- @return table 本次调用的视觉模型状态
function M.new(config)
    if not config.vision_screening_enabled then return {enabled=false} end
    local ok, info = pcall(sai.vision.info)
    if not ok then return {enabled=true, error=tostring(info)} end
    if info == nil or info == sai.json.null then return {enabled=false} end
    return {enabled=true, info=info}
end

--- 【网页搜图】【筛选提示】保持原图片筛选规则和输出格式
--- @param query string 用户查询
--- @param item table 搜索候选
--- @return string 用户消息正文
function M.prompt(query, item)
    return "用户想看的图片：" .. query .. "\n搜索结果标题：" .. item.title .. "\n搜索结果来源：" .. item.page_url
        .. "\n搜索结果描述：" .. item.search_description .. '\n\n请判断这张已下载图片是否适合作为用户要看的图片。只输出 JSON，不要 Markdown，不要解释到 JSON 外面。格式：{"accepted": true, "description": "用中文客观描述图片内容", "reason": "接受或拒绝原因"}'
end

--- 【网页搜图】【布尔兼容】保留原布尔、字符串与整数判定，其他类型默认接受
--- @param value any 模型 accepted 字段
--- @return boolean 是否接受图片
local function accepted(value)
    if type(value) == "boolean" then return value end
    if type(value) == "string" then
        local denied = {["false"]=true, ["0"]=true, no=true, reject=true, rejected=true, ["不"]=true, ["否"]=true, ["拒绝"]=true}
        return not denied[sai.text.trim(value):lower()]
    end
    if type(value) == "number" and math.type(value) == "integer" then return value ~= 0 end
    return true
end

--- 【网页搜图】【模型解析】优先解析 JSON 对象，非 JSON 正文保持原接受与摘录策略
--- @param content string 模型正文
--- @param info table 可信模型标识
--- @return table 完整筛选结果
function M.parse(content, info)
    local raw = sai.text.trim(content)
    local first, last = raw:find("{", 1, true), raw:match(".*()}")
    if first and last and last >= first then
        local ok, data = pcall(sai.json.decode, raw:sub(first, last))
        if ok and type(data) == "table" then
            local description = data.description
            if description == nil then description = data.caption end
            return {status="success", accepted=accepted(data.accepted),
                description=sai.text.trim(text.string(description)), reason=sai.text.trim(text.string(data.reason)),
                provider_id=info.provider_id, model=info.model, error=""}
        end
    end
    return {status="success", accepted=true, description=text.clean(raw, 1600),
        reason="vision model did not return JSON; kept image", provider_id=info.provider_id, model=info.model, error=""}
end

--- 【网页搜图】【单图筛选】图片由原始缓冲发送给宿主选定的视觉模型，失败时保留下载结果
--- @param state table 本次视觉状态
--- @param query string 用户查询
--- @param item table 下载后的图片与缓冲
--- @return table 完整筛选结果
function M.review(state, query, item)
    if not state.enabled then return M.empty("not_requested") end
    if state.error then return M.empty("failed", state.info, state.error) end
    if item.size_bytes > 10 * 1024 * 1024 then
        return M.empty("failed", state.info, "image too large for vision screening: " .. item.size_bytes .. " bytes")
    end
    local ok, response = pcall(item.data.analyze_image, item.data, {
        system="你是图片搜索结果筛选器。只根据图片实际内容判断是否匹配用户想看的图片。",
        prompt=M.prompt(query, item.candidate), mime_type=item.mime_type,
    })
    if not ok then return M.empty("failed", state.info, tostring(response)) end
    return M.parse(response.content, response)
end

return M
