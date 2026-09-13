local values = require("values")
local paths = require("paths")
local library = require("library")
local images = require("images")
local M = {}

--- 【表情显示】【公开入口】找到当前启用图片并返回原元数据，GIF 仍为静态预览
--- @param args table 标识、库名和尺寸
--- @param ctx table 原始 JSON 参数上下文
--- @param config table 显示设置
--- @return table 原版显示结果，animation_note 显式保留 null
function M.run(args, ctx, config)
    local name, id = paths.selected(args, config), values.required(args, "id")
    local loaded = assert(library.find(config, name, id), "meme not found: " .. id)
    images.display(library.display_path(loaded), args, config, ctx)
    local item = loaded.item
    return {success=true, library=name, id=item.id, name=item.name, description=item.description,
        animated=item.animated, animation_note=item.animated and not config.allow_gif_animation
            and "GIF was rendered as a static terminal preview; animation is disabled by default." or sai.json.null}
end

return M
