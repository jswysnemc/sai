# Lua 编辑器提示

[types/](types/) 提供 LuaLS 声明，覆盖入门模板、项目笔记和 URL 预览所用的公共接口。声明按命名空间拆分，只参与补全、悬停和静态诊断；宿主不会自动加载它们。

## 配置

使用支持 LuaLS 的编辑器，将整个 `types/` 目录复制到插件源码目录旁的 `sai-lua-types/`。在插件根目录创建 `.luarc.json`：

```json
{
  "runtime.version": "Lua 5.4",
  "workspace.library": ["../sai-lua-types"],
  "workspace.checkThirdParty": false
}
```

不要在插件中 `require` 这些声明，也不要将它们列入清单入口；`.luarc.json` 是开发配置，安装器不复制它。更新宿主后，使用同一文档版本的声明，避免编辑器提示了旧构建尚不支持的接口。

给业务函数标注上下文即可获得参数和返回值提示：

```lua
--- 【示例插件】【进度】报告当前命令所属的会话
---@param arguments string 用户输入的完整参数文本
---@param ctx SaiContext 当前命令上下文
---@return string 会话标识
local function status(arguments, ctx)
    ctx.progress("Reading session status")
    return ctx.session_id
end
```

工具上下文使用 `SaiToolContext`，比命令多 `json_integer`。`tool_result` 的数据可标为 `SaiToolResultEvent`；事件始终遵守只读及服务限制。`sai.binary.request` 返回 `SaiBinaryResponse`，其 `body:document` 提示转换模式、字符上限及 `SaiDocument` 结果。

## 覆盖范围

| 声明文件 | 当前覆盖 |
| --- | --- |
| [sai.lua](types/sai.lua) | 插件 ID、设置、资源视图及下列命名空间 |
| [registration.lua](types/registration.lua) | 工具、用户命令、普通事件订阅和调用上下文 |
| [data.lua](types/data.lua) | JSON、trim/lower/collapse_whitespace/clip、摘要、UTC 时间 |
| [files.lua](types/files.lua) | 文本读取和规范路径 |
| [storage.lua](types/storage.lua) | 插件持久记录的读取、写入和比较交换 |
| [http.lua](types/http.lua) | 共用请求字段和文本响应 |
| [binary.lua](types/binary.lua) | 原始请求、字节构造、缓冲长度/文本/关闭及正文转换 |

这些声明不是全部 v1 API。模型、工具组合、回复策略、通知、调度、SQLite、进程以及其他文件和缓冲方法继续查阅[能力清单](capability-matrix.md)。使用未声明的接口时可在自己的独立声明目录补充类型，不能据编辑器缺少提示判断宿主是否支持。

LuaLS 能发现拼写、参数类型和部分缺失字段，但不能证明清单授权、数字上下界、嵌套 JSON 可序列化性、工具 Schema、调用阶段或资源预算有效。标准库提示也不表示沙箱开放了 `io/os/package/debug`。继续使用 `sai plugins check` 校验实际加载，并执行代表性的授权和业务调用；版本兼容见[维护规则](compatibility.md)。
