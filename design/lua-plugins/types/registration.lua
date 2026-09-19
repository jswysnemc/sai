---@meta

--- 工具和命令共用的可信上下文视图，字段修改不影响实际权限
---@class SaiContext
---@field session_id string 宿主会话标识
---@field operation_id string 同一次用户操作及组合调用共享的标识
---@field workdir string 本次任务的真实工作目录
---@field allow_writes boolean 本次回调实际获得的写入许可
---@field progress fun(text: string): nil 报告进度，单条最多 4096 字节、每次回调最多 128 条

--- 工具与普通事件另有原始 JSON 整数查询，用户命令不提供此方法
---@class SaiToolContext: SaiContext
---@field json_integer fun(pointer: string): string? 返回 JSON Pointer 对应整数的十进制原文，缺失或非整数返回 nil

---@alias SaiToolAccess 'read_only'|'writes'|'optional_writes'
---@alias SaiCommandAccess 'read_only'|'writes'

--- 宿主提供的完整权限审核事实
---@class SaiPermissionAuditInput
---@field tool string 实际工具名
---@field arguments table<string, any> 完整参数对象
---@field arguments_json string 保留超大整数精度的完整 JSON 参数文本
---@field context string 近期上下文摘要
---@field policy string 宿主审核规则

---@class SaiPermissionAuditOutput
---@field decision 'allow'|'deny'|'abstain' 仅针对当前请求的决定
---@field reason? string 最多 2048 字节且不含控制字符的说明

---@class SaiPermissionAuditDefinition
---@field review fun(input: SaiPermissionAuditInput, ctx: SaiContext): SaiPermissionAuditOutput? 错误或 nil 交还人工

--- 工具定义由宿主在加载时完整校验，参数 Schema 必须描述对象
---@class SaiToolDefinition
---@field name string 包内名称，最多 48 字节，外部完整工具名还受 64 字节限制
---@field description string 模型可见说明
---@field parameters table<string, any> 对象 JSON Schema，仅允许本地引用
---@field access? SaiToolAccess 默认 read_only
---@field execute fun(arguments: table<string, any>, ctx: SaiToolContext): any 返回字符串或可序列化 JSON 值；nil 表示空文本

--- 用户命令接收完整参数文本，不自动进行 shell 解析
---@class SaiCommandDefinition
---@field name string 包内名称，最多 48 字节
---@field description string 用户可见说明
---@field access? SaiCommandAccess 默认 read_only
---@field execute fun(arguments: string, ctx: SaiContext): any 返回字符串或可序列化 JSON 值；nil 表示空文本

---@alias SaiEventName 'agent_start'|'agent_end'|'turn_start'|'turn_end'|'message_start'|'message_end'|'tool_call'|'tool_result'|'reply_end'|'tui_status'

--- tool_result 的有界观察数据，监听器返回值不会修改真实结果
---@class SaiToolResultEvent
---@field name string 实际工具名称
---@field ok boolean 宿主确认的执行结果
---@field output string 最多 16384 个字符的工具输出
