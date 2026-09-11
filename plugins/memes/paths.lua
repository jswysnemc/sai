local values = require("values")
local M = {}

--- 【表情库】【子路径校验】拒绝绝对路径、父目录跳转和跨平台路径分隔歧义
--- @param relative string 索引中的相对文件或默认库名
--- @return string 校验后的相对路径
function M.relative(relative)
    assert(type(relative) == "string" and relative ~= "" and #relative <= 2048,
        "invalid meme relative path")
    assert(not relative:find("[%z\1-\31\\:]") and relative:sub(1, 1) ~= "/",
        "invalid meme relative path")
    for part in relative:gmatch("[^/]+") do
        assert(part ~= "." and part ~= "..", "invalid meme relative path")
    end
    assert(not relative:find("//", 1, true) and relative:sub(-1) ~= "/", "invalid meme relative path")
    return relative
end

--- 【表情库】【目录组合】固定根目录可由可信兼容配置提供，子路径必须先校验
--- @param root string 授权根目录
--- @param relative string 安全相对路径
--- @return string 完整路径
function M.join(root, relative) return root:gsub("/+$", "") .. "/" .. M.relative(relative) end

--- 【表情库】【库选择】显式非空输入经过清洗，旧默认库保持原字符串
--- @param args table 工具参数
--- @param config table 插件设置
--- @return string 所选库名称
function M.selected(args, config)
    local explicit = sai.text.trim(values.string(args.library))
    if explicit ~= "" then return values.sanitize(explicit) end
    return config.libraries.default or "sai"
end

--- 【表情库】【用户目录】用户覆盖层始终使用清洗后的单目录库名
--- @param config table 设置
--- @param library string 库名
--- @return string 用户库路径
function M.user(config, library) return M.join(config.user_dir, values.sanitize(library)) end

--- 【表情库】【内置目录】依次检查开发目录和安装目录，保留环境覆盖的单一来源
--- @param config table 含优先目录数组的设置
--- @param library string 库名
--- @return string 内置库目录
function M.builtin(config, library)
    M.relative(library)
    for number, root in ipairs(config.builtin_dirs) do
        local path = M.join(root, library)
        if number == #config.builtin_dirs then return path end
        local stat = sai.fs.stat(path)
        if stat and stat.is_dir then return path end
    end
end

--- 【表情库】【发送记录路径】沿用旧版本按默认库保存记录的位置
--- @param config table 插件设置
--- @return string 自动发送状态文件
function M.state(config)
    return M.join(config.state_dir, values.sanitize(M.selected({}, config)) .. "/auto-send.json")
end

return M
