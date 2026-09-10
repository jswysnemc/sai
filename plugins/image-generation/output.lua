local M = {}

--- 【图片生成】【文件名称】生成最多 48 个 ASCII 字符的名称前缀
--- @param prompt string 图片提示词
--- @return string 规范前缀，空内容回退为 image
function M.slug(prompt)
    local value = prompt:gsub("[^a-zA-Z0-9 \t\n\f\r_-]", ""):lower():gsub("[ \t\n\f\r_-]", "-"):gsub("%-+", "-"):gsub("^%-", ""):gsub("%-$", "")
    return value ~= "" and value:sub(1, 48) or "image"
end

--- 【图片生成】【自动预览】仅在显示工具可用时调用，禁用显示包会跳过预览
--- @param enabled boolean 是否自动预览
--- @param path string 已保存图片路径
--- @return boolean 是否完成预览
--- @return string|nil 预览失败原因
local function preview(enabled, path)
    if not enabled then return false, nil end
    local ok, tools = pcall(sai.tools.list)
    if not ok then return false, tostring(tools) end
    for _, tool in ipairs(tools) do
        if tool.name == "print_image" then
            local printed, err = pcall(sai.tools.call, "print_image", {image=path}, {timeout_ms=20000})
            return printed, not printed and tostring(err) or nil
        end
    end
    return false, nil
end

--- 【图片生成】【保存结果】保存成功后才预览，预览错误不改变文件生成状态
--- @param config table 设置
--- @param prompt string 提示词
--- @param image userdata 图片字节句柄
--- @return table 保持原工具返回字段的结果
function M.save(config, prompt, image)
    local directory = sai.text.trim(config.output_dir)
    if directory == "" then directory = "." end
    local filename = M.slug(prompt) .. "-" .. sai.time.local_format("%Y%m%d-%H%M%S") .. ".png"
    local separator = directory:find("[/\\]$") and "" or "/"
    local ok, file = pcall(image.write, image, directory .. separator .. filename)
    image:close()
    if not ok then error(file) end
    local printed, print_error = preview(config.auto_print, file.path)
    return {
        status="ok", path=file.path, final_response_must_include_path=file.path, bytes=file.bytes,
        printed=printed, print_error=print_error or sai.json.null,
        assistant_instruction=printed
            and "The generated image has already been printed in the terminal. In your final response, include the exact local image path from final_response_must_include_path. Do not call print_image again unless the user asks to print it again."
            or "The generated image was saved to disk. In your final response, include the exact local image path from final_response_must_include_path. Do not call print_image unless the user explicitly asked to display, print, render, preview, or show it.",
    }
end

return M
