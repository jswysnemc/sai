local paths = require("paths")
local snapshot = require("storage.snapshot")
local transaction = require("storage.transaction")
local import = require("mutations.import")
local M = {}

--- 【知识库导入】【递归目录】完整枚举后再导入，截断或深度超限不能伪装成成功
--- @param root string 已授权输入目录
--- @param prefix string 相对导入前缀
--- @param output table 待导入文件列表
--- @param depth integer 当前目录深度
--- @return nil 原地追加普通文件，链接和特殊文件不跟随
local function collect(root, prefix, output, depth)
    assert(depth <= 64, "knowledge base import exceeds directory depth limit")
    local directory = sai.fs.read_dir(root, {max_entries=1024})
    assert(not directory.truncated, "knowledge base import directory exceeds entry limit")
    for _, entry in ipairs(directory.entries) do
        local relative = prefix .. "/" .. entry.name
        if entry.is_dir then collect(entry.path, relative, output, depth + 1)
        elseif entry.is_file then
            assert(#output < 4096, "knowledge base import exceeds file count limit")
            output[#output + 1] = {path=entry.path, name=paths.relative(relative)}
        end
    end
end

--- 【知识库导入】【源文件读取】完整读取并校验来源，暂不修改知识库文件
--- @param config table 配置
--- @param source table 来源路径及目标名称
--- @return string 已通过文件格式校验的 UTF-8 正文
local function load(config, source)
    local content = snapshot.with(source.path, config.max_file_bytes, function(buffer)
        assert(buffer, "source file not found")
        return buffer:bytes(0, buffer:len())
    end)
    paths.validate(config, source.name, content)
    return content
end

--- 【知识库导入】【单项提交】正文已经通过来源校验，事务错误必须向调用方报告
--- @param config table 配置
--- @param source table 来源及目标名称
--- @param content string 已验证正文
--- @return string 已提交名称
local function save(config, source, content)
    return transaction.with(config, true, function() return import.content(config, source.name, content) end)
end

--- 【知识库导入】【管理入口】单文件错误向上传递，目录内不支持的文件沿用逐项跳过行为
--- @param config table 配置
--- @param path string 已授权来源
--- @param requested_name string|nil CLI 保留的原名称
--- @return table 成功导入的路径数组
function M.add(config, path, requested_name)
    transaction.with(config, true, function() end)
    local info = assert(sai.fs.stat(path), "source file not found")
    local name = requested_name or paths.name(path:gsub("/$", ""))
    assert(name ~= "" and name ~= "." and name ~= "..", info.is_dir and "source directory has no valid directory name" or "source file has no valid file name")
    if not info.is_dir then
        local source = {path=path, name=paths.relative(name)}
        return sai.json.array({save(config, source, load(config, source))})
    end
    local files, added = {}, sai.json.array()
    collect(path, name, files, 1)
    for _, file in ipairs(files) do
        local ok, content = pcall(load, config, file)
        if ok then added[#added + 1] = save(config, file, content)
        else
            local problem = tostring(content)
            if problem:find("budget", 1, true) or problem:find("cancelled", 1, true) or problem:find("timed out", 1, true) then
                error(content, 0)
            end
        end
    end
    return added
end

return M
