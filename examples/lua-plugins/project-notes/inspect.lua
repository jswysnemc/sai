local M = {}

-- 1. 【项目笔记】【设置校验】初始化只检查设置，文件读取留在回调中执行
for key in pairs(sai.config) do
    assert(key == "path" or key == "preview_chars", "unknown project-notes setting: " .. key)
end
local path = sai.config.path
if path == nil then path = "notes/README.md" end
assert(type(path) == "string" and #path > 0 and #path <= 4096,
    "project-notes path must be a nonempty string of at most 4096 bytes")
local preview_chars = sai.config.preview_chars
if preview_chars == nil then preview_chars = 160 end
assert(type(preview_chars) == "number" and preview_chars % 1 == 0
    and preview_chars >= 1 and preview_chars <= 1000,
    "project-notes preview_chars must be an integer between 1 and 1000")

--- 【项目笔记】【正文检查】读取有界 UTF-8 文件并生成完整正文摘要和预览
---@param args table 已通过工具 Schema 校验的空对象
---@param ctx SaiContext 宿主工作目录、会话和进度上下文
---@return table 规范路径、正文字节数、SHA-256 和预览
function M.run(args, ctx)
    ctx.progress("Reading project notes")
    -- 1. 【项目笔记】【读取边界】不把截断正文的摘要作为完整文件摘要
    local file = sai.fs.read_text(path, { max_bytes = 65536, lossy = false })
    assert(not file.truncated, "project-notes file exceeds 65536 bytes")
    -- 2. 【项目笔记】【结果组合】只组合公共路径、文本和摘要接口
    return {
        path = sai.fs.realpath(path),
        bytes = #file.text,
        sha256 = sai.crypto.digest("sha256", file.text),
        preview = sai.text.clip(sai.text.collapse_whitespace(file.text), preview_chars),
    }
end

--- 【项目笔记】【命令检查】通过用户命令复用同一文件检查逻辑
---@param arguments string 命令参数，必须为空
---@param ctx SaiContext 宿主调用上下文
---@return table 文件检查结果
function M.command(arguments, ctx)
    assert(sai.text.trim(arguments) == "", "inspect takes no arguments; use plugins configure")
    return M.run({}, ctx)
end

return M
