local http = require("http")
local candidate = require("candidate")
local text = require("strings")
local M = {}

--- 【网页搜图】【字段回退】仅缺失字段时回退，JSON null 和 false 保持原优先级
--- @param object table 引擎字段
--- @param first string 首选键
--- @param second string 备用键
--- @return any 选中的字段
local function field(object, first, second)
    if object[first] ~= nil then return object[first] end
    return object[second]
end

--- 【网页搜图】【Bing 解析】沿用 iusc 与 m 属性扫描顺序，逐个解析候选 JSON
--- @param html string 搜索页面正文
--- @param limit integer 候选池上限
--- @return table 候选数组
function M.parse(html, limit)
    local candidates, offset = sai.json.array(), 1
    while true do
        local anchor = html:find("<a", offset, true)
        if not anchor then break end
        local class = html:find('class="iusc"', anchor, true)
        if not class then offset = anchor + 2 else
            local marker = html:find('m="', class, true)
            if not marker then offset = class + 1 else
                local first = marker + 3
                local last = html:find('"', first, true)
                if not last then break end
                local ok, data = pcall(sai.json.decode, text.unescape(html:sub(first, last - 1)))
                if ok and type(data) == "table" then
                    local value = candidate.build(field(data, "t", "desc"), data.purl, data.murl, data.turl,
                        "Bing Images", field(data, "w", "expw"), field(data, "h", "exph"), data.desc)
                    if value then candidates[#candidates + 1] = value end
                end
                if #candidates >= limit then break end
                offset = last
            end
        end
    end
    return candidates
end

--- 【网页搜图】【Bing 请求】作为主来源不足时的第二来源，保留安全搜索参数
--- @param config table 包设置
--- @param query string 用户查询
--- @param limit integer 候选池上限
--- @param safe boolean 是否启用安全搜索
--- @return table 候选数组
function M.search(config, query, limit, safe)
    local fields = {{"q", query}, {"first", "1"}}
    if safe then fields[#fields + 1] = {"safeSearch", "Strict"} end
    return M.parse(http.get(config, config.bing_base_url .. "/images/search?" .. http.query(fields)), limit)
end

return M
