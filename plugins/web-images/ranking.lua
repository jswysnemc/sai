local M = {}
local generic = { ["图片"]=true, ["照片"]=true, ["高清"]=true, ["壁纸"]=true,
    photo=true, image=true, images=true, picture=true, wallpaper=true, hd=true, ["4k"]=true }

--- 【网页搜图】【查询词】按 Unicode 空白与 ASCII 标点分词，去掉原通用搜图词
--- @param query string 用户查询
--- @return table 保持原顺序的查询词
function M.terms(query)
    local parts = {}
    for index = 1, #query do
        local byte = query:byte(index)
        local punctuation = (byte >= 33 and byte <= 47) or (byte >= 58 and byte <= 64)
            or (byte >= 91 and byte <= 96) or (byte >= 123 and byte <= 126)
        parts[index] = punctuation and " " or query:sub(index, index)
    end
    local result = sai.json.array()
    for term in sai.text.collapse_whitespace(table.concat(parts)):lower():gmatch("[^ ]+") do
        if #term >= 2 and not generic[term] then result[#result + 1] = term end
    end
    return result
end

--- 【网页搜图】【候选评分】保持标题命中、尺寸、噪声与头像规则
--- @param query string 用户查询
--- @param item table 图片候选
--- @return number 原权重得分
function M.score(query, item)
    local title = item.title:lower()
    local metadata = (item.title .. " " .. item.page_url .. " " .. item.image_url):lower()
    local score = 0
    for _, term in ipairs(M.terms(query)) do
        if title:find(term, 1, true) then score = score + 24
        elseif metadata:find(term, 1, true) then score = score + 10 end
    end
    local short = math.min(item.width, item.height)
    score = score + (short >= 900 and 28 or short >= 600 and 24 or short >= 300 and 18 or short >= 100 and 4 or -8)
    if item.width * (item.height + 0.0) >= 1000000 then score = score + 7 end
    for _, term in ipairs({"thumb", "thumbnail", "sprite", "placeholder", "banner", "advert", "favicon"}) do
        if metadata:find(term, 1, true) then score = score - 8; break end
    end
    if metadata:find("avatar", 1, true) and not query:find("头像", 1, true) and not query:lower():find("avatar", 1, true) then
        score = score - 8
    end
    return score
end

--- 【网页搜图】【地址去重】忽略查询参数和 ASCII 大小写，保留首个候选
--- @param candidates table 合并后的候选数组
--- @return table 去重后的候选数组
function M.dedupe(candidates)
    local result, seen = sai.json.array(), {}
    for _, item in ipairs(candidates) do
        local key = item.image_url:match("^[^?]*"):lower()
        if not seen[key] then result[#result + 1], seen[key] = item, true end
    end
    return result
end

--- 【网页搜图】【稳定排序】分数相同时保持搜索引擎原顺序
--- @param query string 用户查询
--- @param candidates table 待排序数组
--- @return table 新的有序候选数组
function M.rank(query, candidates)
    local ranked = {}
    for index, item in ipairs(candidates) do ranked[index] = {item=item, score=M.score(query, item), index=index} end
    table.sort(ranked, function(left, right)
        return left.score > right.score or (left.score == right.score and left.index < right.index)
    end)
    local result = sai.json.array()
    for _, entry in ipairs(ranked) do result[#result + 1] = entry.item end
    return result
end

--- 【网页搜图】【候选预算】按用户数量扩大候选池，最多三十条
--- @param count integer 最终请求数量
--- @return integer 候选池上限
function M.pool_limit(count)
    return math.max(count, math.min(math.max(count * 4, count + 8), 30))
end

--- 【网页搜图】【下载预算】限制尝试数量，同时保留小请求的替代候选
--- @param count integer 最终请求数量
--- @return integer 下载尝试上限
function M.probe_limit(count)
    return math.max(count, math.min(math.max(count * 4, count + 6), 16))
end

return M
