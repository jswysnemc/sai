local paths = require("paths")
local values = require("values")
local transaction = require("storage.transaction")
local M = {}

--- 【知识库导入】【完整文件】调用方持有数据锁，校验之后才记录事务
--- @param config table 配置
--- @param name string 相对路径
--- @param content string 完整 UTF-8 字节
--- @return string 已保存路径
function M.content(config, name, content)
    name = paths.relative(name)
    paths.validate(config, name, content)
    transaction.commit(config, {kind="write", name=name, content=content, clear_semantic=config.embedding_enabled})
    return name
end

--- 【知识库上传】【标题文件名】保留原 ASCII 标题规则及本地时钟后备名称
--- @param title string 已去除首尾空白的标题
--- @return string 最多 48 个字符的名称
local function slug(title)
    local parts = {}
    for _, code in utf8.codes(title) do
        local ch = utf8.char(code)
        if code < 128 and ch:match("[%w]") then parts[#parts + 1] = values.lower(ch)
        elseif ch == "-" or ch == "_" or sai.text.trim(ch) == "" then parts[#parts + 1] = "-" end
    end
    local text = table.concat(parts):gsub("%-+", "-"):gsub("^%-", ""):gsub("%-$", "")
    return text == "" and ("note-" .. sai.time.local_format("%H%M%S")) or text:sub(1, 48)
end

--- 【知识库上传】【用途边界】保持知识资料与技能、记忆、身份和配置分离
--- @param content string 正文
--- @param title string 标题
--- @param name string 显式名称
--- @return nil 命中原拒绝词时抛出错误
local function validate_purpose(content, title, name)
    local text = values.lower(content .. "\n" .. title .. "\n" .. name)
    for _, word in ipairs({"skill", "skills/", "skll", "记忆", "memory", "persona", "identity", "prompt", "配置", "config"}) do
        assert(not text:find(word, 1, true), "this content looks like a skill, memory, prompt, identity, or config request; do not upload it to the knowledge base")
    end
end

--- 【知识库上传】【Markdown 保存】保留来源、标题与本地上传时间
--- @param config table 配置
--- @param args table 正文、可选标题与文件名
--- @return table 原公开成功结果
function M.upload(config, args)
    assert(config.upload_tool_enabled, "knowledge base upload tool is disabled")
    local content = values.required(args, "content")
    local title = type(args.title) == "string" and sai.text.trim(args.title) or "knowledge note"
    local name = type(args.file_name) == "string" and sai.text.trim(args.file_name) or ""
    validate_purpose(content, title, name)
    name = name == "" and ("chat_uploads/" .. sai.time.local_format("%Y-%m-%d") .. "/" .. slug(title) .. ".md") or paths.relative(name)
    local heading = title
    if heading == "" then heading = paths.stem(name) end
    local body = "# " .. heading .. "\n\n> 来源：用户要求保存到本地知识库\n> 上传时间：" .. sai.time.local_format("%Y-%m-%d %H:%M:%S") .. "\n\n" .. content .. "\n"
    return transaction.with(config, true, function() return {ok=true, path=M.content(config, name, body)} end)
end

return M
