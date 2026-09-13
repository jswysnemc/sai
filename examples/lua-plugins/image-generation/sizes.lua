local M = {}
local rightcode = {
    ["3:2"]={"1536x1024", "2048x1365"}, ["16:9"]={"1536x864", "2048x1152"},
    ["4:3"]={"1365x1024", "2048x1536"}, ["5:4"]={"1280x1024", "2048x1638"},
    ["21:9"]={"1536x658", "2048x878"}, ["2:3"]={"1024x1536", "1365x2048"},
    ["3:4"]={"1024x1365", "1536x2048"}, ["9:16"]={"864x1536", "1152x2048"},
    ["4:5"]={"1024x1280", "1638x2048"},
}
local portrait = {["2:3"]=true, ["3:4"]=true, ["9:16"]=true, ["4:5"]=true}
local landscape = {["3:2"]=true, ["16:9"]=true, ["4:3"]=true, ["5:4"]=true, ["21:9"]=true}

--- 【图片生成】【模型识别】沿用模型名称中 gpt-image 的大小写不敏感判断
--- @param model string 模型名称
--- @return boolean 是否使用 GPT Image 的请求规则
function M.gpt_image(model)
    return model:lower():find("gpt-image", 1, true) ~= nil
end

--- 【图片生成】【尺寸映射】保留 OpenAI 与 RightCode 的比例和分辨率映射
--- @param config table 配置
--- @param ratio string 宽高比
--- @param resolution string 分辨率
--- @return string|nil 请求尺寸，RightCode 自动尺寸时省略
function M.resolve(config, ratio, resolution)
    if config.provider_type == "rightcode" then
        if ratio == "1:1" then return "1024x1024" end
        local pair = rightcode[ratio]
        if pair then return pair[(resolution == "2K" or resolution == "4K") and 2 or 1] end
        return nil
    end
    if M.gpt_image(config.model) then
        if portrait[ratio] then return "1024x1536" end
        if landscape[ratio] then return "1536x1024" end
        return ratio == "1:1" and "1024x1024" or "auto"
    end
    if portrait[ratio] then return "1024x1792" end
    if landscape[ratio] then return "1792x1024" end
    return "1024x1024"
end

return M
