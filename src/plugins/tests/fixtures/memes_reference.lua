local values = require("values")
local metadata = require("metadata")
local settings = require("settings")
local images = require("images")
local paths = require("paths")
local search = require("search")

--- 【表情对照】【规则分发】调用实际发布模块，保留测试输入原始 JSON 整数
--- @param input table 原版样本输入
--- @param ctx table 可信上下文
--- @return any 规则结果；空结果使用显式 JSON null
local function evaluate(input, ctx)
    local args = input.args or {}
    local operation = input.operation
    if operation == "sanitize" then return values.sanitize(input.value) end
    if operation == "normalize" then return values.normalize(input.value) end
    if operation == "ids_match" then return values.ids_match(input.stored, input.requested) end
    if operation == "display_name" then return values.display_name(input.name) end
    if operation == "json_slice" then return values.json_slice(input.value) or sai.json.null end
    if operation == "score" then return search.score(input.item, input.query, values.strings(input.tags)) end
    if operation == "extension" then return images.extension(input.path) end
    if operation == "metadata" then
        return metadata.manual(args, "sha256:fixture", "images/fixture.png", "image/png", false)
    end
    if operation == "vision_metadata" then
        return metadata.vision(args, "sha256:fixture", "images/fixture.png", "image/png", false)
    end
    if operation == "update" then return metadata.update(input.item, args) end
    if operation == "library" then return paths.selected(args, settings.load({})) end
    if operation == "size" then
        --- 【表情对照】【输入定位】只改变测试样本包裹层，不重新解析 JSON 数值
        --- @param pointer string 参数内 JSON Pointer
        --- @return string|nil 原始整数
        local function integer(pointer) return ctx.json_integer("/args" .. pointer) end
        return settings.size(args, settings.load({}), {columns=80,rows=24}, {json_integer=integer}) or sai.json.null
    end
    error("unknown reference operation")
end

--- 【表情对照】【结果编码】字符串和 null 也以 JSON 输出，避免工具文本转换丢失类型
--- @param input table 样本输入
--- @param ctx table 可信上下文
--- @return string JSON 结果
local function execute(input, ctx) return sai.json.encode(evaluate(input, ctx)) end
sai.register_tool({name="reference",description="Frozen native meme rules",parameters={type="object"},execute=execute})
