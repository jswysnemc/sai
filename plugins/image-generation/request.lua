local sizes = require("sizes")
local M = {}
local MAX_RESPONSE = 32 * 1024 * 1024

--- 【图片生成】【错误摘要】折叠空白并按字符限制长度，保持原来的省略号格式
--- @param text string 响应正文
--- @param limit integer 最多保留的字符数
--- @return string 有界错误摘要
function M.preview(text, limit)
    text = sai.text.collapse_whitespace(text)
    local end_at = utf8.offset(text, limit + 1)
    return end_at and (text:sub(1, end_at - 1) .. "...") or text
end

--- 【图片生成】【请求正文】组装供应商参数，GPT Image 不发送 response_format
--- @param config table 配置
--- @param prompt string 已去除首尾空白的提示词
--- @param ratio string 宽高比
--- @param resolution string 分辨率
--- @return table JSON 请求对象
function M.payload(config, prompt, ratio, resolution)
    local result = {model=config.model, prompt=prompt, n=1, size=sizes.resolve(config, ratio, resolution)}
    if config.provider_type == "rightcode" then
        if ratio ~= "" and ratio ~= "自动" then result.aspect_ratio = ratio end
    elseif not sizes.gpt_image(config.model) then
        result.response_format = "b64_json"
    end
    return result
end

--- 【图片生成】【结果提取】优先解码 Base64，再执行无凭据图片 URL 下载
--- @param response table API 响应，正文为受控二进制句柄
--- @param timeout integer 本次请求超时毫秒数
--- @return userdata 图片字节句柄
local function extract(response, timeout)
    local body = response.body
    assert(body:json_type("/data") == "array", "image response missing data array")
    assert(body:json_type("/data/0") ~= nil, "image response data is empty")
    local image = body:json_base64("/data/0/b64_json")
    if image then return image end
    local url = body:json_string("/data/0/url")
    assert(url ~= nil, "image response contains neither b64_json nor url")
    body:close()
    local downloaded = sai.binary.download({url=url, timeout_ms=timeout, max_bytes=MAX_RESPONSE})
    if downloaded.status < 200 or downloaded.status >= 300 then
        downloaded.body:close()
        error("failed to download generated image (" .. downloaded.status .. ")")
    end
    return downloaded.body
end

--- 【图片生成】【请求执行】首个非空配置密钥用于 API 请求，错误和下载均不转发该密钥
--- @param config table 生成设置
--- @param prompt string 提示词
--- @param ratio string 宽高比
--- @param resolution string 分辨率
--- @return userdata 可写入文件的图片句柄
function M.run(config, prompt, ratio, resolution)
    local key
    for _, candidate in ipairs(config.api_keys) do
        candidate = sai.text.trim(candidate)
        if candidate ~= "" then key = candidate; break end
    end
    assert(key, "plugins.image_generation.api_keys is empty")
    local base = sai.text.trim(config.base_url):gsub("/+$", "")
    assert(base ~= "", "plugins.image_generation.base_url is empty")
    local timeout = config.timeout_seconds * 1000
    local response = sai.binary.request({url=base .. "/v1/images/generations", method="POST",
        headers={authorization="Bearer " .. key, ["content-type"]="application/json"},
        body=sai.json.encode(M.payload(config, prompt, ratio, resolution)),
        timeout_ms=timeout, max_bytes=MAX_RESPONSE})
    if response.status < 200 or response.status >= 300 then
        local text = M.preview(response.body:text(4096), 500)
        response.body:close()
        error("image API error (" .. response.status .. "): " .. text)
    end
    local ok, result = pcall(extract, response, timeout)
    response.body:close()
    if not ok then error(result) end
    return result
end

return M
