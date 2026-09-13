local rules = require("claims")
local M = {}
local tool_name = "fcitx5_input_method_wiki_qurey"
local diagnostic_zh = "本机运行时证据优先；Wiki 只能提供官方一般规则，不能覆盖 /proc、环境变量、已加载模块或实际输入测试。"
local diagnostic_en = "Local runtime evidence comes first; the Wiki provides official general rules and must not override /proc data, environment, loaded modules, or an actual input test."

--- 【Fcitx Wiki】【响应格式】保持原工具的中文包装和英文结构
--- @param topic string 已选择的主题
--- @param query string 已修剪的查询文本
--- @param page table 白名单来源
--- @param language string bilingual、zh 或 en
--- @param excerpt string|userdata 页面摘录或 JSON null
--- @return table 兼容原工具的结构化规则与来源
function M.format(topic, query, page, language, excerpt)
    local claims = sai.json.array()
    for _, rule in ipairs(rules[topic] or {}) do
        if language == "en" then
            claims[#claims + 1] = {
                claim = { en = rule.en }, applies_to = rule.applies_to,
                confidence = "official_wiki_general_rule", caveat = rule.caveat,
            }
        else
            local claim = {
                ["结论"] = rule.zh, ["适用范围"] = rule.applies_to,
                ["可信度"] = "official_wiki_general_rule", ["注意事项"] = rule.caveat,
            }
            if language == "bilingual" then claim.english_reference = rule.en end
            claims[#claims + 1] = claim
        end
    end
    query = query ~= "" and query or sai.json.null
    if language == "en" then
        return {
            ok = true, tool = tool_name,
            spelling_note = "Tool name keeps the requested 'qurey' spelling for compatibility.",
            topic = topic, query = query, source = page, claims = claims,
            diagnostic_rule = { en = diagnostic_en }, page_excerpt = excerpt,
        }
    end
    local output = {
        ["状态"] = true, ["工具"] = tool_name,
        ["说明"] = "这是 Fcitx5 官方 Wiki 专用查询工具；中文为主包装，英文保留为原文参考。",
        ["命名备注"] = "工具名按用户要求保留 qurey 拼写。",
        ["主题"] = topic, ["查询"] = query,
        ["来源"] = { ["标题"] = page.title, ["URL"] = page.url },
        ["官方规则摘录"] = claims, ["诊断使用规则"] = diagnostic_zh, ["页面摘录"] = excerpt,
    }
    if language == "bilingual" then output.diagnostic_rule_en = diagnostic_en end
    return output
end

return M
