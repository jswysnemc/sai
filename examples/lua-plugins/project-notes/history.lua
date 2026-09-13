local inspect = require("inspect")
local M = {}

--- 【项目笔记】【保存检查】保存本插件最近一次完整检查，不写入项目文件
---@param arguments string 命令参数，必须为空
---@param ctx SaiContext 已取得写入许可的宿主上下文
---@return SaiJsonValue 已保存的检查记录
function M.remember(arguments, ctx)
    assert(sai.text.trim(arguments) == "", "remember takes no arguments")
    -- 1. 【项目笔记】【重新读取】每次保存都重新检查当前已授权文件
    local record = inspect.run({}, ctx)
    record.checked_at = sai.time.utc_now().rfc3339
    record.schema_version = 1
    -- 2. 【项目笔记】【私有持久化】宿主独立检查插件存储授权和写入许可
    return sai.storage.plugin.set("latest", record)
end

--- 【项目笔记】【读取记录】查询跨会话保存的最近检查
---@param arguments string 命令参数，必须为空
---@param ctx SaiContext 只读宿主上下文
---@return table 包含最近记录，缺失时 record 为 JSON null
function M.latest(arguments, ctx)
    assert(sai.text.trim(arguments) == "", "latest takes no arguments")
    return { record = sai.storage.plugin.get("latest") }
end

--- 【项目笔记】【清除记录】显式删除本插件保存的检查记录
---@param arguments string 命令参数，必须为空
---@param ctx SaiContext 已取得写入许可的宿主上下文
---@return table 表示记录已经清除
function M.forget(arguments, ctx)
    assert(sai.text.trim(arguments) == "", "forget takes no arguments")
    sai.storage.plugin.set("latest", nil)
    return { cleared = true }
end

return M
