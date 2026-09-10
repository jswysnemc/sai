local metadata = require("image_metadata")
local ranking = require("ranking")
local screening = require("screening")
local text = require("strings")
local M = {}

--- 【网页搜图】【单候选下载】依次尝试原图和缩略图，识别类型后按摘要原子保存
--- @param config table 包设置
--- @param candidate table 图片候选
--- @return table|nil 下载结果及仍有效的图片缓冲
function M.candidate(config, candidate)
    local urls = {{candidate.image_url, false}}
    if candidate.thumbnail_url ~= "" and candidate.thumbnail_url ~= candidate.image_url then
        urls[#urls + 1] = {candidate.thumbnail_url, true}
    end
    local max_bytes = math.floor(math.min(64, math.max(0.1, config.max_download_mb)) * 1024 * 1024)
    for _, url in ipairs(urls) do
        local ok, response = pcall(sai.binary.download, {url=url[1], max_bytes=max_bytes,
            timeout_ms=math.max(5, config.timeout_seconds) * 1000})
        if ok then
            local body = response.body
            if response.status >= 200 and response.status < 300 and body:len() > 0 then
                local final_url = response.url ~= "" and response.url or url[1]
                local mime = metadata.mime(body, response.headers["content-type"] or "", final_url)
                if mime then
                    local width, height = metadata.dimensions(body, mime)
                    if width > 0 and height > 0 then candidate.width, candidate.height = width, height end
                    local digest = body:sha256()
                    local path = config.cache_dir:gsub("[/\\]+$", "") .. "/webimg-" .. digest .. metadata.extension(mime)
                    local saved = body:write(path)
                    return {candidate=candidate, local_path=saved.path, mime_type=mime, size_bytes=body:len(),
                        sha256=digest, used_thumbnail=url[2], data=body}
                end
            end
            body:close()
        end
    end
    return nil
end

--- 【网页搜图】【下载与筛选】按原尝试上限去重，拒绝结果继续寻找候选
--- @param config table 包设置
--- @param ctx table 当前宿主上下文
--- @param query string 用户查询
--- @param candidates table 排序后的候选
--- @param count integer 最终请求数量
--- @param vision table 视觉状态
--- @return table, integer 接受的图片数组及视觉拒绝数量
function M.run(config, ctx, query, candidates, count, vision)
    local images, hashes, rejected = sai.json.array(), {}, 0
    for index = 1, math.min(#candidates, ranking.probe_limit(count)) do
        if #images >= count then break end
        ctx.progress(text.language(config, "downloading images", "正在下载图片") .. string.format(" %d/%d", #images + 1, count))
        local item = M.candidate(config, candidates[index])
        if item then
            if not hashes[item.sha256] then
                hashes[item.sha256] = true
                if vision.enabled then
                    ctx.progress(text.language(config, "reviewing images", "正在审核图片") .. string.format(" %d/%d", #images + 1, count))
                end
                item.vision = screening.review(vision, query, item)
                if item.vision.status == "success" and not item.vision.accepted then
                    rejected = rejected + 1
                    ctx.progress(text.language(config, "image rejected by review", "图片审核已拒绝") .. " " .. rejected)
                else
                    images[#images + 1] = item
                    ctx.progress(text.language(config, "accepted images", "已通过图片") .. string.format(" %d/%d", #images, count))
                end
            end
            item.data:close()
            item.data = nil
        end
    end
    assert(#images > 0, "image search found candidates, but no image could be downloaded")
    return images, rejected
end

return M
