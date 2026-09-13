local config = require("settings").load(sai.config or {})
local search = require("search")
local download = require("download")
local screening = require("screening")
local result = require("result")
local text = require("strings")

--- 【网页搜图】【调用入口】根据宿主写入权限选择远程元数据或完整下载筛选流程
--- @param args table 查询数量及预览选项
--- @param ctx table 宿主提供的权限和进度上下文
--- @return table 图片结果
local function execute(args, ctx)
    local query = sai.text.trim(args.query)
    assert(query ~= "", "query is required")
    assert(type(args.count) == "number" and args.count >= 0 and
        (math.type(args.count) == "integer" or args.count >= 2.0 ^ 63),
        "count is required; choose the number of images from the user's request")
    local count = math.floor(math.min(math.max(args.count, 1), math.min(math.max(config.max_results, 1), 10)))
    local safe, preview = config.safe_search, config.auto_preview
    if args.safe_search ~= nil then safe = args.safe_search end
    if args.preview ~= nil then preview = args.preview end
    local preview_count = args.preview_count
    if type(preview_count) ~= "number" or math.type(preview_count) ~= "integer" or preview_count < 0 then preview_count = count end
    preview_count = math.min(preview_count, count, 5)
    ctx.progress(text.language(config, "searching image candidates", "正在搜索图片候选"))
    local candidates = search.run(config, query, count, safe)
    if not ctx.allow_writes then
        while #candidates > count do table.remove(candidates) end
        return {success=#candidates > 0, query=query, count=#candidates, mode="metadata_only", images=candidates}
    end
    local vision = screening.new(config)
    local images, rejected = download.run(config, ctx, query, candidates, count, vision)
    return result.finish(config, ctx, query, images, rejected, vision, preview, preview_count)
end

sai.register_tool({
    name="search_web_images", access="optional_writes",
    description=text.language(config,
        "Search web images with DuckDuckGo and Bing fallback. In normal mode it can download selected images to the local cache and optionally preview them in the terminal. In read-only mode it only returns remote image metadata.",
        "搜索网络图片，使用 DuckDuckGo，失败或不足时回退 Bing。普通模式可下载选中图片到本地缓存并可在终端预览；只读模式只返回远程图片元数据。"),
    parameters={type="object", properties={
        query={type="string", description="Image search query."},
        count={type="integer", description="Required. Exact number of images to return. Match the user's requested quantity: one/a/an/一张/一幅 means 1; a few/几张 means 3; several/多张 means 5 unless the user gives another number. Do not use the configured maximum as the default."},
        preview={type="boolean", description="Download and preview images when terminal image printing is enabled."},
        preview_count={type="integer", description="Maximum images to preview in the terminal."},
        safe_search={type="boolean", description="Enable safe image search. Defaults to plugin config."},
    }, required=sai.json.array({"query", "count"}), additionalProperties=false},
    execute=execute,
})
