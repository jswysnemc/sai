local M = {}

--- 【网页搜图】【字节展示】保持原二进制单位与一位小数格式
--- @param bytes integer 图片大小
--- @return string 可读字节数
function M.bytes(bytes)
    local value = bytes + 0.0
    for _, unit in ipairs({"B", "KB", "MB", "GB"}) do
        if value < 1024 or unit == "GB" then
            return unit == "B" and (bytes .. " B") or string.format("%.1f %s", value, unit)
        end
        value = value / 1024
    end
end

--- 【网页搜图】【图片结果】只返回可复用路径与元数据，不序列化原始缓冲
--- @param item table 已下载并完成筛选的图片
--- @return table 原公开结果字段
function M.image(item)
    local result = {}
    for key, value in pairs(item.candidate) do result[key] = value end
    for _, key in ipairs({"local_path", "mime_type", "size_bytes", "sha256", "used_thumbnail", "vision"}) do
        result[key] = item[key]
    end
    result.size_human = M.bytes(item.size_bytes)
    return result
end

--- 【网页搜图】【预览与报告】调用独立显示包，显示失败仍保留已经保存的图片
--- @param config table 包设置
--- @param ctx table 当前宿主上下文
--- @param query string 用户查询
--- @param stored table 已保存图片
--- @param rejected integer 视觉拒绝数量
--- @param vision table 视觉状态
--- @param preview boolean 是否请求预览
--- @param preview_count integer 预览上限
--- @return table 完整工具报告
function M.finish(config, ctx, query, stored, rejected, vision, preview, preview_count)
    local visible = false
    if preview and preview_count > 0 then
        for _, tool in ipairs(sai.tools.list()) do
            if tool.name == "print_image" then visible = true; break end
        end
    end
    local errors = sai.json.array()
    if visible then
        ctx.progress("__external_output__")
        for index = 1, math.min(#stored, preview_count) do
            local ok, error = pcall(sai.tools.call, "print_image", {image=stored[index].local_path})
            if not ok then errors[#errors + 1] = stored[index].local_path .. ": " .. tostring(error) end
        end
    end
    local images = sai.json.array()
    for _, item in ipairs(stored) do images[#images + 1] = M.image(item) end
    return {success=#images > 0, query=query, count=#images, result_role="downloaded_image_candidates",
        vision_screening=vision.enabled and "enabled" or "unavailable",
        description_policy="vision.description is produced by the configured vision model after download; search_description is only search-engine metadata. Prefer vision.description when explaining whether an image matches the request.",
        rejected_by_vision=rejected, cache_dir=stored[1].local_path:match("^(.*)[/\\]") or config.cache_dir,
        printed=visible and #errors == 0 and #images > 0, print_errors=errors, images=images,
        assistant_instruction=visible
            and "The searched images have been downloaded and previewed in the terminal when possible. In your final response, include the local_path values for reusable images. Do not call print_image again for already printed images unless the user asks."
            or "The searched images have been downloaded to local_path. In your final response, include useful local_path and page_url values. Call print_image only if the user explicitly asks to render or preview them.",
    }
end

return M
