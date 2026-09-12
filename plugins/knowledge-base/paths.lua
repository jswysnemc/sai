local values = require("values")
local M = {}

--- 【知识库路径】【相对路径】规范分隔符和点组件，拒绝绝对路径、父目录及空白名称
--- @param value string 输入路径
--- @return string 规范知识库相对路径
function M.relative(value)
    value = sai.text.trim(value)
    local windows = sai.system.platform == "windows"
    assert(value:sub(1, 1) ~= "/" and not (windows and (value:match("^%a:") or value:sub(1, 1) == "\\")),
        "knowledge base path must be relative")
    if windows then value = value:gsub("\\", "/") end
    local parts = {}
    for part in value:gmatch("[^/]+") do
        assert(part ~= "..", "knowledge base path contains illegal component")
        if part ~= "." then
            assert(not part:find("\0", 1, true) and sai.text.trim(part) ~= "", "invalid path component")
            parts[#parts + 1] = part
        end
    end
    assert(#parts > 0, "knowledge base path is empty")
    return table.concat(parts, "/")
end

--- 【知识库路径】【路径组合】所有业务文件只从已校验的库内名称产生
--- @param config table 配置
--- @param name string 相对路径
--- @return string 文件完整路径
function M.file(config, name) return config.files_dir .. "/" .. M.relative(name) end

--- 【知识库路径】【文件名称】沿用以正斜线分隔的公开输出
--- @param path string 相对路径
--- @return string 最后一段名称
function M.name(path) return path:match("([^/]*)$") end

--- 【知识库路径】【标题后备】只去除最后的扩展名，单个前导点不视为扩展名
--- @param path string 相对文件路径
--- @return string 与原文件主名规则一致的标题
function M.stem(path)
    local name = M.name(path)
    local dot = name:match("^.*()%.")
    return dot and dot > 1 and name:sub(1, dot - 1) or name
end

--- 【知识库路径】【目录名称】无上级目录时返回空文字
--- @param path string 相对路径
--- @return string 父目录
function M.directory(path) return path:match("^(.*)/") or "" end

--- 【知识库校验】【配置集合】逗号分隔规则只做 ASCII 大小写归一
--- @param text string 配置列表
--- @return table 去重名称集合
local function csv(text)
    local result = {}
    for part in (text .. ","):gmatch("(.-),") do
        part = values.lower(sai.text.trim(part))
        if part ~= "" then result[part] = true end
    end
    return result
end

--- 【知识库校验】【文件内容】拒绝空文件、超限字节、无效 UTF-8 和不允许的文件名
--- @param config table 配置
--- @param name string 相对路径
--- @param content string 原始字节
--- @return nil 校验失败时抛出原业务错误
function M.validate(config, name, content)
    assert(#content > 0, "file is empty")
    assert(#content <= config.max_file_bytes, "file too large: " .. #content .. " bytes")
    assert(utf8.len(content), "file is not valid UTF-8 text")
    local base = values.lower(M.name(name))
    local extension = base:match("^.+(%.([^%.]*))$")
    assert((extension and csv(config.allowed_extensions)[extension]) or csv(config.allowed_filenames)[base],
        "unsupported file type or name: " .. base)
end

return M
