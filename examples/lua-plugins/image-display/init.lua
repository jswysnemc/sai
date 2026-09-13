local settings = require("settings")
local config = settings.load(sai.config or {})

--- 【图片显示】【文本选择】使用当前插件语言返回稳定文案
--- @param en string 英文文案
--- @param zh string 中文文案
--- @return string 当前语言文案
local function text(en, zh)
    return config.language == "zh" and zh or en
end

--- 【图片显示】【工具执行】处理路径和尺寸后调用宿主绘制，结果只包含图片位置
--- @param args table 图片路径与可选尺寸
--- @return string 本地化的显示完成信息
local function display(args)
    local path = sai.text.trim(args.image or "")
    assert(path ~= "", text("image is required", "缺少图片路径"))
    local size = settings.size(args, config, sai.terminal.size())
    local result = sai.terminal.display_image(path, size)
    return text("printed image in terminal", "已在终端打印图片") .. ": " .. result.path
end

sai.register_tool({
    name="print_image", access="read_only",
    description=text("Print/render a local image directly in the current terminal output using terminal image protocols or an ANSI fallback. Use this when the user asks to show, print, render, or preview an image, or when you need to inspect an image visually in the terminal before answering.", "使用终端图片协议或 ANSI 降级在当前终端输出中直接打印/渲染本地图片。当用户要求显示、打印、渲染、预览图片，或回答前需要在终端中目视检查图片时使用。"),
    parameters={type="object", properties={
        image={type="string", description=text("Local image path.", "本地图片路径。")},
        size={type="string", description=text("Optional terminal size, e.g. 80x40. Use this or width/height to avoid oversized output.", "可选终端显示尺寸，例如 80x40。用它或 width/height 避免输出过大。")},
        width={type="integer", description=text("Optional output width in terminal cells, e.g. 80.", "可选终端单元格输出宽度，例如 80。")},
        height={type="integer", description=text("Optional output height in terminal cells, e.g. 40.", "可选终端单元格输出高度，例如 40。")},
    }, required=sai.json.array({"image"}), additionalProperties=false},
    execute=display,
})
