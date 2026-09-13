local values = require("values")
local paths = require("paths")
local index = require("index")
local library = require("library")
local images = require("images")
local metadata = require("metadata")
local M = {}

--- 【表情新增】【已有结果】构造原版重复添加结果，图片路径经过内置覆盖解析
--- @param name string 库名
--- @param loaded table 已有条目
--- @return table 原版公开结果
local function duplicate(name, loaded)
    return {success=true, already_exists=true, library=name, id=loaded.item.id,
        name=loaded.item.name, path=library.image_path(loaded)}
end

--- 【表情新增】【元数据准备】只在未提供任何字段时调用独立视觉服务
--- @param args table 输入
--- @param buffer userdata 原图片缓冲
--- @param id string 内容标识
--- @param file string 目标相对路径
--- @param ext string 标准扩展名
--- @return table|nil 完整元数据
--- @return table|nil 视觉请求或解析失败的兼容结果
local function describe(args, buffer, id, file, ext)
    local mime, animated = images.mime(ext), ext == "gif"
    if metadata.supplied(args) then return metadata.manual(args, id, file, mime, animated) end
    local ok, data = pcall(function()
        local response = buffer:analyze_image({system="请基于图片内容回答，不要编造看不见的信息。",
            prompt=require("prompts").vision, mime_type=mime})
        assert(sai.text.trim(response.content) ~= "", "vision model returned empty response")
        return sai.json.decode(values.json_slice(response.content) or response.content)
    end)
    if not ok then
        return nil, {success=false, needs_user_info=true,
            message="vision metadata generation failed; ask the user what the image shows and when to use it, then call add_meme again with metadata fields",
            error=tostring(data)}
    end
    return metadata.vision(data, id, file, mime, animated)
end

--- 【表情新增】【独占图片】新图片使用随机后缀，避免删除重试误删同内容的后续添加
--- @param buffer userdata 图片字节
--- @param base string 用户库根目录
--- @param hash string 内容摘要
--- @param ext string 扩展名
--- @return string 相对文件名
--- @return string 目标路径
local function create_image(buffer, base, hash, ext)
    for _ = 1, 4 do
        local file = "images/" .. hash:sub(1, 16) .. "-" .. index.token() .. "." .. ext
        local target = paths.join(base, file)
        if buffer:write_if(target, nil) then return file, target end
    end
    error("could not reserve a new meme image filename")
end

--- 【表情新增】【清理未引用文件】只移除本次独占创建但没有进入索引的图片
--- @param target string 新图片路径
--- @return nil 失败时报告需要人工处理的位置
local function cleanup(target)
    local ok, err = pcall(sai.fs.remove_file, target)
    assert(ok, "unreferenced meme image remains at " .. target .. ": " .. tostring(err))
end

--- 【表情新增】【发布流程】先验证元数据，再创建图片，最后以条件更新合并索引
--- @param args table 输入
--- @param config table 设置
--- @param name string 库名
--- @param source string 输入路径
--- @param buffer userdata 图片缓冲
--- @return table 新增或重复结果
local function add(args, config, name, source, buffer)
    local hash = buffer:sha256()
    local id, base = "sha256:" .. hash, paths.user(config, name)
    local index_path = paths.join(base, "index.json")
    local current = index.load(index_path, name)
    assert(not index.pending(current, id), "meme deletion is pending; retry delete_meme first")
    local existing = library.find(config, name, id, current)
    if existing then return duplicate(name, existing) end
    local ext = images.extension(source)
    local item, failure = describe(args, buffer, id, "images/pending." .. ext, ext)
    if failure then return failure end
    local file, target = create_image(buffer, base, hash, ext)
    buffer:close()
    item.file = file
    local ok, result = pcall(index.mutate, index_path, name, function(latest)
        assert(not index.pending(latest, id), "meme deletion is pending; retry delete_meme first")
        local found = library.find(config, name, id, latest)
        if found then return duplicate(name, found), false end
        local retired = {}
        for _, old in ipairs(latest.memes) do
            if values.ids_match(old.id, id) and old.file ~= file then retired[#retired + 1] = paths.join(base, old.file) end
        end
        latest.disabled_ids = index.without(latest.disabled_ids, function(value) return values.ids_match(value, id) end)
        latest.memes = index.without(latest.memes, function(value) return values.ids_match(value.id, id) end)
        latest.memes[#latest.memes + 1] = values.copy(item)
        return {success=true, library=name, id=id, name=item.name, path=target, metadata=item, _retired=retired}
    end)
    if not ok then cleanup(target); error(result, 0) end
    if result.already_exists then cleanup(target) end
    -- 3. 【表情新增】【替换清理】只清理已从本次成功索引修订中移除的旧图片
    local retired = result._retired or {}
    result._retired = nil
    for _, old_path in ipairs(retired) do
        local removed, err = pcall(sai.fs.remove_file, old_path)
        if not removed then
            result.cleanup_error = tostring(err)
            result.unreferenced_path = old_path
            break
        end
    end
    return result
end

--- 【表情新增】【公开入口】以可信写入权限执行，任何退出路径都关闭源图片缓冲
--- @param args table 图片路径和元数据
--- @param ctx table 可信权限上下文
--- @param config table 设置
--- @return table 原版兼容结果
function M.run(args, ctx, config)
    assert(ctx.allow_writes, "read-only callback cannot add memes")
    local name, source = paths.selected(args, config), values.required(args, "image")
    local buffer = images.read(source, config)
    local ok, result = pcall(add, args, config, name, source, buffer)
    buffer:close()
    if not ok then error(result, 0) end
    return result
end

return M
