local settings = require("settings").parse(sai.config)
local format = require("format")

--- 【底栏插件】【状态渲染】按设置组织宿主提供的状态，不读取文件或调用外部服务
--- @param event table 当前模型、模式、用量与终端状态
--- @return table 左右两段纯文本
local function render(event)
    return format.render(event, settings)
end

sai.on("tui_status", render)
