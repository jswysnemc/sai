local currencies = require("currencies")
local settings = require("settings")
local M = {}

--- 【汇率查询】【接口请求】只向清单授权来源发送请求，错误不包含带密钥的 URL 或响应正文
--- @param url string 固定来源和编码参数组成的地址
--- @return table|any 解码后的 JSON 响应；HTTP 或解析失败直接结束调用
local function request(url)
    local response = sai.http.request({method="GET", url=url, timeout_ms=30000, max_bytes=1048576})
    assert(response.status >= 200 and response.status < 300, "exchange rate HTTP status " .. response.status)
    return sai.json.decode(response.text)
end

--- 【汇率查询】【数值选择】按原版字段规则读取数值，不能把 false、null 或字符串当作汇率
--- @param data any 接口结果
--- @param field string 汇率对象字段
--- @param target string 目标币种
--- @return number|nil 有限数值汇率
local function rate(data, field, target)
    local rates = type(data) == "table" and data[field] or nil
    local value = type(rates) == "table" and rates[target] or nil
    if type(value) == "number" then return value end
end

--- 【汇率查询】【结果格式】沿用原版浮点数格式，不使用 Lua 默认的有限有效数字展示
--- @param base string 基础币种
--- @param target string 目标币种
--- @param value number 汇率
--- @return string 用户可读的汇率结果
local function result(base, target, value)
    return base .. " 到 " .. target .. " 的汇率是: " .. sai.text.number_to_string(value)
end

--- 【汇率查询】【查询编排】优先配置接口，仅在有效 JSON 未给出成功汇率时使用允许的免费回退
--- @param args table 包含 base 和 target 的参数
--- @return string 汇率文本；未授权、请求失败、缺少币种或禁用回退时失败
function M.run(args)
    local base, target = currencies.code(args.base), currencies.code(args.target)
    assert(base ~= "" and target ~= "", "base and target are required")
    if settings.api_key ~= "" then
        local data = request("https://v6.exchangerate-api.com/v6/" .. sai.text.url_encode(settings.api_key)
            .. "/latest/" .. sai.text.url_encode(base))
        if type(data) == "table" and data.result == "success" then
            local value = rate(data, "conversion_rates", target)
            if value ~= nil then return result(base, target, value) end
        end
    end
    assert(settings.free_fallback_enabled, "exchange rate API key failed or missing and free fallback is disabled")
    local data = request("https://open.er-api.com/v6/latest/" .. sai.text.url_encode(base))
    local value = rate(data, "rates", target)
    assert(value ~= nil, "target currency not found: " .. target)
    return result(base, target, value)
end

return M
