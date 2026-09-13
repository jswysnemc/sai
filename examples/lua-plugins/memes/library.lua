local values = require("values")
local paths = require("paths")
local index = require("index")
local M = {}

--- 【表情库】【禁用查询】沿用存储标识对完整条目标识的单向匹配
--- @param identifiers table 禁用或覆盖标识
--- @param id string 完整条目标识
--- @return boolean 是否包含匹配项
function M.contains(identifiers, id)
    for _, stored in ipairs(identifiers) do
        if values.ids_match(stored, id) then return true end
    end
    return false
end

--- 【表情库】【覆盖合并】用户条目优先，待删除项不展示，内置图片保留可用回退路径
--- @param config table 设置
--- @param library string 库名
--- @param user table|nil 当前条件修改使用的用户索引
--- @param include_disabled boolean|nil 是否供启用更新查询禁用项
--- @return table 按原顺序合并的记录
function M.load(config, library, user, include_disabled)
    local builtin_dir, user_dir = paths.builtin(config, library), paths.user(config, library)
    local builtin = index.load(paths.join(builtin_dir, "index.json"), library)
    user = user or index.load(paths.join(user_dir, "index.json"), library)
    local result, user_ids = {}, {}
    for _, item in ipairs(user.memes) do
        user_ids[#user_ids + 1] = item.id
        if (include_disabled or not M.contains(user.disabled_ids, item.id)) and not index.pending(user, item.id) then
            local loaded = {item=item, source="user", path=paths.join(user_dir, item.file), root=user_dir}
            for _, base in ipairs(builtin.memes) do
                if values.ids_match(base.id, item.id) and base.file == item.file then
                    loaded.fallback = paths.join(builtin_dir, base.file)
                    loaded.fallback_root = builtin_dir
                    break
                end
            end
            result[#result + 1] = loaded
        end
    end
    for _, item in ipairs(builtin.memes) do
        if (include_disabled or not M.contains(user.disabled_ids, item.id))
            and not M.contains(user_ids, item.id) and not index.pending(user, item.id) then
            result[#result + 1] = {item=item, source="builtin", path=paths.join(builtin_dir, item.file), root=builtin_dir}
        end
    end
    return result
end

--- 【表情库】【条目定位】按原先顺序选择第一个请求前缀匹配项
--- @param config table 设置
--- @param library string 库名
--- @param id string 请求标识
--- @param user table|nil 当前用户索引
--- @param include_disabled boolean|nil 是否允许读取禁用项
--- @return table|nil 带来源及路径的记录
function M.find(config, library, id, user, include_disabled)
    for _, loaded in ipairs(M.load(config, library, user, include_disabled)) do
        if values.ids_match(loaded.item.id, id) then return loaded end
    end
end

--- 【表情库】【图片定位】只有匹配内置标识及文件名的元数据覆盖才回退到内置图片
--- @param loaded table 已合并记录
--- @return string 实际显示路径
--- @return boolean 是否使用内置图片
function M.image_path(loaded)
    if loaded.fallback then
        local stat = sai.fs.stat(loaded.path)
        if not stat then return loaded.fallback, true end
        assert(stat.is_file, "meme image path is not a file")
    end
    return loaded.path, loaded.source == "builtin"
end

--- 【表情库】【显示路径】解析真实路径，拒绝指向所属库 images 目录以外的符号链接
--- @param loaded table 当前启用条目及其库目录
--- @return string 经过读取授权和目录归属检查的绝对图片路径
function M.display_path(loaded)
    local path, builtin = M.image_path(loaded)
    local root = builtin and (loaded.fallback_root or loaded.root) or loaded.root
    local directory = paths.for_comparison(sai.fs.realpath(root)):gsub("/+$", "") .. "/images/"
    local absolute = sai.fs.realpath(path)
    assert(paths.for_comparison(absolute):sub(1, #directory) == directory, "meme image escaped library images directory")
    return absolute
end

return M
