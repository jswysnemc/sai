local schema = require("state_schema")
local M = {}

--- 【待办导入】【显式来源】只接受用户传入的完整状态或已授权快照文件
--- @param text string 包含 state 或 path 的 JSON 对象
--- @return table 已验证的旧状态副本
local function read(text)
    local args = sai.json.decode(text)
    assert(type(args) == "table", "todo import requires a JSON object")
    for key in pairs(args) do
        assert(key == "state" or key == "path", "unknown todo import argument")
    end
    assert((args.state ~= nil) ~= (args.path ~= nil), "provide exactly one of state or path")
    local value = args.state
    if args.path ~= nil then
        assert(type(args.path) == "string" and args.path ~= "", "invalid todo import path")
        local file = sai.fs.read_text(args.path, {max_bytes=262144, lossy=false})
        assert(not file.truncated, "todo import exceeds 256 KiB")
        value = sai.json.decode(file.text)
    end
    assert(type(value) == "table", "todo import state must be an object")
    local state = schema.decode(value)
    assert(#sai.json.encode(state) <= 262144, "todo import exceeds 256 KiB")
    return state
end

--- 【待办导入】【空计划接续】以比较交换提交，禁止覆盖现有条目或历史
--- @param text string 明确的导入参数
--- @param ctx SaiContext 可信会话上下文
--- @return table 导入条目与历史批次数，原快照保持原样
function M.run(text, ctx)
    assert(ctx.allow_writes, "todo import requires write permission")
    local imported = read(text)
    for _ = 1, 16 do
        -- 1. 【待办导入】【覆盖保护】并发修改后重新检查，空视图初始化不阻止导入
        local previous = sai.storage.get("plan")
        local current = schema.decode(previous)
        assert(#current.items == 0 and #current.history == 0, "todo import requires an empty plan and history")
        -- 2. 【待办导入】【原子提交】文件读取不附带删除或回写旧快照的操作
        if sai.storage.compare_exchange("plan", previous, imported) then
            return {ok=true, items=#imported.items, history=#imported.history}
        end
    end
    error("todo state changed too often; retry the import")
end

return M
