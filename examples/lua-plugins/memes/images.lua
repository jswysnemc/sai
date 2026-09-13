local values = require("values")
local settings = require("settings")
local M = {}

--- 【表情库】【图片扩展名】只检查最后一个文件组件，保留 JPEG 别名
--- @param path string 图片路径
--- @return string 标准 jpg、png、webp 或 gif 扩展名
function M.extension(path)
    local file = path:match("[^/]+$") or ""
    local ext = values.lower(file:match("^.+%.([^.]*)$") or "")
    if ext == "jpeg" then ext = "jpg" end
    assert(ext == "jpg" or ext == "png" or ext == "webp" or ext == "gif",
        "unsupported image extension: " .. ext .. "; supported: jpg, jpeg, png, webp, gif")
    return ext
end

--- 【表情库】【媒体类型】扩展名由 extension 校验，本函数返回固定 MIME
--- @param ext string 标准扩展名
--- @return string 媒体类型
function M.mime(ext) return ext == "jpg" and "image/jpeg" or "image/" .. ext end

--- 【表情库】【受限读取】完整读取普通文件，使用原配置大小和宿主二进制预算
--- @param source string 原图片路径
--- @param config table 插件设置
--- @return userdata 原始字节缓冲
--- @return string 已规范图片路径
function M.read(source, config)
    -- 1. 【表情库】【大小预算】使用浮点乘法避免合法 u64 配置在 Lua 有符号整数中溢出
    local limit = config.max_image_mb * 1048576.0
    local stat = sai.fs.stat(source)
    assert(stat and stat.is_file, "image path is not a file: " .. source)
    assert(stat.len <= limit,
        "image too large: " .. tostring(stat.len) .. " bytes; limit is " .. tostring(config.max_image_mb) .. " MiB")
    local absolute = sai.fs.realpath(source)
    -- 2. 【表情库】【完整读取】巨大配置不会扩大宿主预算，实际读取仍限制为可表示的整数
    local budget = math.tointeger(math.max(1, math.min(limit, sai.limits.binary_bytes)))
    local buffer = sai.binary.read_file(absolute, {max_bytes=budget})
    assert(buffer:len() <= limit, "image grew beyond configured size limit")
    return buffer, absolute
end

--- 【表情库】【终端显示】沿用旧尺寸规则，终端协议由宿主执行
--- @param path string 经索引解析的图片路径
--- @param args table 显示参数
--- @param config table 显示设置
--- @param ctx table 原始参数上下文
--- @return table 宿主显示结果
function M.display(path, args, config, ctx)
    return sai.terminal.display_image(path, settings.size(args, config, sai.terminal.size(), ctx))
end

return M
