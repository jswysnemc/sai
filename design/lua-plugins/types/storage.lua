---@meta

---@class SaiStorage
---@field plugin SaiPluginStorage 同一插件跨会话的独立持久记录

--- 操作均要求 system.plugin_storage；写入与删除另需本次回调写入许可
---@class SaiPluginStorage
---@field get fun(key: string): SaiJsonValue 返回已保存的 JSON 值，缺失时为 JSON null
---@field set fun(key: string, value: SaiJsonValue): SaiJsonValue 保存并返回该值；nil 或 JSON null 删除记录
---@field compare_exchange fun(key: string, expected: SaiJsonValue, value: SaiJsonValue): boolean 当前 JSON 值与 expected 相等时原子替换，返回是否匹配
