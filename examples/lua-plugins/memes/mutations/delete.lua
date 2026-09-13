local values = require("values")
local paths = require("paths")
local index = require("index")
local library = require("library")
local M = {}

--- 【表情删除】【准备记录】条件发布待删除记录；内置图片只调整禁用状态
--- @param config table 设置
--- @param name string 库名
--- @param path string 用户索引
--- @param args table 用户标识和显式删除方式
--- @return table 删除计划或内置禁用结果
local function prepare(config, name, path, args)
    local requested = values.required(args, "id")
    return index.mutate(path, name, function(current)
        local pending = index.pending(current, requested)
        if pending then
            local mode = args.hard_delete == nil and pending.mode or (args.hard_delete and "remove" or "trash")
            local changed = pending.mode ~= mode
            pending.mode = mode
            return {deletion=values.copy(pending)}, changed
        end
        local found = assert(library.find(config, name, requested, current, true), "meme not found: " .. requested)
        local _, builtin = library.image_path(found)
        if builtin then
            if not library.contains(current.disabled_ids, found.item.id) then
                current.disabled_ids[#current.disabled_ids + 1] = found.item.id
            end
            return {success=true, library=name, id=found.item.id, action="disabled_builtin_meme"}
        end
        assert(#current.pending_deletions < 64, "too many pending meme deletions")
        pending = {id=found.item.id, file=found.item.file, token=index.token(), mode=args.hard_delete and "remove" or "trash"}
        current.pending_deletions[#current.pending_deletions + 1] = pending
        return {deletion=values.copy(pending)}
    end)
end

--- 【表情删除】【索引完成】只消费本次记录对应的原文件，不删除后来添加的新条目
--- @param path string 用户索引
--- @param name string 库名
--- @param deletion table 已执行文件操作的记录
--- @return table 原版删除结果
local function finish(path, name, deletion)
    return index.mutate(path, name, function(current)
        local pending = index.pending(current, deletion.id)
        local result = {success=true, library=name, id=deletion.id, action="deleted_user_meme"}
        if not pending or pending.token ~= deletion.token then return result, false end
        assert(pending.file == deletion.file, "pending meme file changed")
        current.memes = index.without(current.memes, function(item)
            return item.id == deletion.id and item.file == deletion.file
        end)
        current.pending_deletions = index.without(current.pending_deletions, function(item) return item.token == deletion.token end)
        current.disabled_ids = index.without(current.disabled_ids, function(id) return values.ids_match(id, deletion.id) end)
        return result
    end)
end

--- 【表情删除】【公开入口】先标记再删除最后提交；取消或错误保留显式可恢复状态
--- @param args table 标识及可选永久删除选项
--- @param ctx table 可信权限
--- @param config table 设置
--- @return table 删除或禁用结果
function M.run(args, ctx, config)
    assert(ctx.allow_writes, "read-only callback cannot delete memes")
    local name = paths.selected(args, config)
    local base = paths.user(config, name)
    local path = paths.join(base, "index.json")
    -- 1. 【表情删除】【条件标记】后续读取和修改不会引用待删除图片
    local plan = prepare(config, name, path, args)
    if not plan.deletion then return plan end
    -- 2. 【表情删除】【单文件操作】回收站错误不会改为永久删除，缺失文件可继续完成重试
    local deletion = plan.deletion
    local remove = deletion.mode == "remove" and sai.fs.remove_file or sai.fs.trash_file
    remove(paths.join(base, deletion.file))
    -- 3. 【表情删除】【条件完成】匹配原记录后移除元数据，并保留其他并发修改
    return finish(path, name, deletion)
end

return M
