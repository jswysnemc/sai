local config = require("settings").load(sai.config or {})
local request = require("request")
local output = require("output")

--- 【图片生成】【工具执行】验证提示词后依次请求、保存和按配置预览
--- @param args table 提示词及可选比例和分辨率
--- @return table 图片位置与显示状态
local function generate(args)
    local prompt = sai.text.trim(args.prompt or "")
    assert(prompt ~= "", "prompt is required")
    local ratio = sai.text.trim(args.aspect_ratio or config.default_aspect_ratio)
    local resolution = sai.text.trim(args.resolution or config.default_resolution)
    return output.save(config, prompt, request.run(config, prompt, ratio, resolution))
end

sai.register_tool({
    name="generate_image", access="writes",
    description="Generate an image from a text prompt using the configured OpenAI or RightCode image API. Returns a local image path. In the final assistant response, always include the returned path so the user can reuse it. Do not call print_image after this tool unless the user explicitly asks to display/print/preview the image; if this tool returns printed=true, never call print_image for the same image.",
    parameters={type="object", properties={
        prompt={type="string", description="Image generation prompt."},
        aspect_ratio={type="string", enum=sai.json.array({"自动", "1:1", "2:3", "3:2", "3:4", "4:3", "4:5", "5:4", "9:16", "16:9", "21:9"}), description="Optional aspect ratio override."},
        resolution={type="string", enum=sai.json.array({"1K", "2K", "4K"}), description="Optional resolution hint for RightCode."},
    }, required=sai.json.array({"prompt"}), additionalProperties=false},
    execute=generate,
})
