local common = require("providers.common")
local M = {}

--- 【游戏信号】【ProtonDB 评级】已有 App ID 时读取原始评级，失败不替代其他证据
--- @param app_id integer|nil 非负整数 Steam App ID
--- @return any 成功返回的 JSON 值，包括 JSON null；未请求或请求失败时返回 nil
function M.fetch(app_id)
    if app_id == nil then return nil end
    local ok, value = pcall(common.json,
        "https://www.protondb.com/api/v1/reports/summaries/" .. app_id .. ".json")
    if ok then return value end
end

return M
