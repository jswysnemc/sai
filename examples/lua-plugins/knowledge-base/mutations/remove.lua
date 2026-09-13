local transaction = require("storage.transaction")
local paths = require("paths")
local M = {}

--- 【知识库删除】【文件与索引】保留删除缺失文件的幂等行为并移除全部相关块
--- @param config table 配置
--- @param name string 相对路径
--- @return table 原公开删除结果
function M.run(config, name)
    local rel = paths.relative(name)
    return transaction.with(config, true, function()
        transaction.commit(config, {kind="remove", name=rel, clear_semantic=true})
        return {ok=true, path=rel, warning=sai.json.null}
    end)
end

return M
