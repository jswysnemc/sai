local M = {}

--- 【闹钟】【声音路径】校验路径文本和扩展名，不把任意文件类型交给解码器
--- @param value string 音频路径
--- @return string 原路径
function M.check(value)
    assert(type(value) == "string" and #value > 0 and #value <= 8192, "invalid audio_file path")
    local extension = value:match("%.([^./\\]+)$")
    extension = extension and extension:lower()
    assert(extension == "wav" or extension == "mp3", "audio_file must be a .wav or .mp3 file")
    return value
end

--- 【闹钟】【音频准备】通过读取授权解析绝对路径并检查普通文件及大小
--- @param value string|nil 用户输入，空白表示内置声音
--- @return string|userdata 规范路径或 JSON null
function M.resolve(value)
    if value == nil then return sai.json.null end
    value = sai.text.trim(value)
    if value == "" then return sai.json.null end
    -- 1. 【闹钟】【读取边界】真实路径与属性都由宿主在可信目录中校验
    local path = M.check(sai.fs.realpath(value))
    local info = sai.fs.stat(path)
    assert(info and info.is_file, "audio_file is not a regular file")
    assert(info.len > 0 and info.len <= 8 * 1024 * 1024, "audio_file must contain 1 byte to 8 MiB")
    return path
end

return M
