local M = {}

--- 【网页搜图】【实体解码】沿用原有六种实体及替换顺序
--- @param value string 搜索引擎返回的文本
--- @return string 解码后的文本
function M.unescape(value)
    return (value:gsub("&amp;", "&"):gsub("&quot;", '"'):gsub("&#x27;", "'")
        :gsub("&#39;", "'"):gsub("&lt;", "<"):gsub("&gt;", ">"))
end

--- 【网页搜图】【可见文本】按 Unicode 字符截断并折叠空白
--- @param value string 原文本
--- @param limit integer 最多保留的字符数
--- @return string 截断时带三个句点的摘要
function M.clean(value, limit)
    local text = sai.text.collapse_whitespace(M.unescape(value))
    local ending = utf8.offset(text, limit + 1)
    return ending and ending <= #text and (text:sub(1, ending - 1) .. "...") or text
end

--- 【网页搜图】【地址整理】仅清理首尾空白与原有实体，不改写查询参数
--- @param value string 原地址
--- @return string 清理后的地址
function M.url(value)
    return M.unescape(sai.text.trim(value))
end

--- 【网页搜图】【字符字段】非字符串 JSON 字段按空字符串处理
--- @param value any 原字段
--- @return string 原字符串或空字符串
function M.string(value)
    return type(value) == "string" and value or ""
end

--- 【网页搜图】【来源标记】沿用原来源文本提取方式，不执行网络或 DNS 查询
--- @param url string 页面地址
--- @return string|nil 来源主机文本
function M.host(url)
    local start = url:find("://", 1, true)
    return start and url:sub(start + 3):match("^[^/]*"):lower() or nil
end

--- 【网页搜图】【双语文本】按当前包语言选择进度与工具文案
--- @param config table 包设置
--- @param en string 英文文本
--- @param zh string 中文文本
--- @return string 选中的文本
function M.language(config, en, zh)
    return config.language == "zh" and zh or en
end

return M
