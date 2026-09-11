local values = require("values")
local paths = require("paths")
local index = require("index")
local library = require("library")
local metadata = require("metadata")
local M = {}

--- 【表情更新】【合并修改】每次条件冲突重新应用字段修改，禁用项仍可被显式更新
--- @param args table 条目标识和更新字段
--- @param ctx table 可信权限
--- @param config table 设置
--- @return table 更新后的元数据
function M.run(args, ctx, config)
    assert(ctx.allow_writes, "read-only callback cannot update memes")
    local name, requested = paths.selected(args, config), values.required(args, "id")
    local path = paths.join(paths.user(config, name), "index.json")
    return index.mutate(path, name, function(current)
        assert(not index.pending(current, requested), "meme deletion is pending; retry delete_meme first")
        local found = assert(library.find(config, name, requested, current, true), "meme not found: " .. requested)
        local item = metadata.update(values.copy(found.item), args)
        local replaced = false
        for number, old in ipairs(current.memes) do
            if values.ids_match(old.id, item.id) then current.memes[number] = item; replaced = true; break end
        end
        if not replaced then current.memes[#current.memes + 1] = item end
        if args.enabled == true then
            current.disabled_ids = index.without(current.disabled_ids, function(id) return values.ids_match(id, item.id) end)
        elseif args.enabled == false and not library.contains(current.disabled_ids, item.id) then
            current.disabled_ids[#current.disabled_ids + 1] = item.id
        end
        return {success=true, library=name, id=item.id, metadata=item}
    end)
end

return M
