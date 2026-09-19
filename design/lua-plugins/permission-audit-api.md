# 权限审核插件接口

`sai.register_permission_audit({review = function(input, ctx) ... end})` 注册一个可选的审核回调。每个包最多注册一次，声明和实际授权都必须包含 `permission_audit: true`。仅注册回调不会接管审核，宿主还必须通过 `permission.auto_audit_plugin_id` 明确选择插件。

```lua
--- 【权限审核】【回调示例】由插件根据完整事实给出三态决定
--- @param input table 工具事实和审核规则
--- @param ctx table 可信会话、请求和工作目录
--- @return table 本次请求的审核决定
local function review(input, ctx)
    return {decision = "abstain", reason = "等待人工审核"}
end

sai.register_permission_audit({review = review})
```

输入字段：

| 字段 | 含义 |
| --- | --- |
| `tool` | 实际工具名 |
| `arguments` | 完整 JSON 对象，超出 128 KiB 总输入限制时拒绝调用 |
| `arguments_json` | 宿主保留的完整参数 JSON 文本，用于避免 Lua 数值转换舍入超大整数 |
| `context` | 原有自动审核的近期上下文摘要 |
| `policy` | 宿主自动审核规则 |

`ctx.session_id`、`ctx.operation_id`、`ctx.workdir` 必须由宿主提供；操作标识对应待审批请求。`ctx.allow_writes` 固定为 `false`，不提供模型或工具调用服务，也不允许修改私有存储。已授权的精确只读 POST 可用于推理请求。

结果必须是 `{decision = "allow"|"deny"|"abstain", reason = 可选字符串}`。不允许其他字段；说明最多 2048 字节且不能含控制字符。返回 `nil` 等价于弃权，错误结果不能成为批准。宿主通过现有权限代理提交一次性决定，保留人工先决定和请求终止后的竞态语义。

插件使用既有指令、内存、HTTP 及回调时限，宿主总等待上限为 45 秒。插件缺失、未启用、未授权或执行失败时回到人工审核。只有未设置插件 ID 时才使用既有聊天模型审核。

指定 `auto_audit_provider_id` 和 `auto_audit_model` 时，两者必须同时非空；宿主将该供应商的 `id`、`base_url`、`model`、`api_key` 放入本实例的 `sai.config.provider`。凭据沿用 sai 原有解析规则，不持久复制，也不向普通插件实例公开。插件自身的网络授权仍独立校验目标端点。

CLI 可通过 `--allow-permission-audit`、`--no-permission-audit` 独立调整审核能力；这些开关不能与 `--grant-declared` 混用。
