# 插件私有作用域锁

`sai.storage.plugin.with_lock(key, callback, options?)` 协调同一插件的多个实例或进程。它使用独立的 `system.plugin_storage` 授权，不增加普通文件权限，也不改变回调的读写许可。存储记录本身的原子比较交换见[私有状态接口](private-api.md)。

## 接口

以下示例在已授权的写入回调中执行：

```lua
local store = sai.storage.plugin
local count = store.with_lock("counter:update", function()
    local old = store.get("counter")
    if old == nil or old == sai.json.null then old = 0 end
    store.set("counter", old + 1)
    return old + 1
end, {timeout_ms=10000})
```

| 参数或结果 | 契约 |
| --- | --- |
| `key` | 1–256 字节 UTF-8 字符串，不含控制字符；不接受隐式数字转换 |
| `callback` | 无参数函数，可以调用受当前许可约束的异步宿主接口 |
| `options.timeout_ms` | 默认 10000，范围 1–600000 毫秒；未知选项拒绝 |
| 返回值 | 原样返回回调的全部结果，包括中间的 nil |

锁等待和回调执行共用外层的取消、指令与总时长限制。一次合法取锁消耗一次系统调用额度，回调中的文件、存储等调用继续分别计费。初始化阶段与无效回调拒绝取锁；已授权只读工具、命令和事件可以用锁协调读取，持久记录和普通文件写入仍需要各自许可。

## 归属与嵌套

锁命名空间位于应用状态根目录的 `plugin-locks/<插件摘要>/<空作用域摘要>/`。不同插件隔离；同一状态根下的同一插件跨工作区、会话、Lua VM 和进程共享锁。该目录与 `plugin-storage`、`plugin-state` 分开，使用锁并不创建对应键的 JSON 存储记录。

最多嵌套四层，键必须严格递增，按 UTF-8 字符串顺序比较。同键重入和反序等待在进入宿主前拒绝。例如先取得 `knowledge:embedding`，再取得 `knowledge:store` 合法；反向顺序拒绝。这个约束只协调本接口的锁，不替调用方管理其他外部锁。

每插件最多创建 128 个不同锁键。目录注册使用短 catalog 锁，实际键对应稳定锁文件；文件不会在释放锁时删除，也不会为同一键更换 inode。达到键数上限后仍可使用已有键。状态目录、锁和中间路径遵循私有目录的无链接约束，实际锁文件必须是普通文件。

## 释放、超时与取消

运行时在 Rust 中持有租约，不向 Lua 返回锁句柄。回调成功、抛错、取消或外层超时都会释放租约和嵌套次序。保存回调函数不会保留锁；退出后新实例可以再次取得同一键。

正式宿主使用操作系统文件锁等待竞争，权限和文件结构错误立即失败。超时返回 `plugin lock acquisition timed out`。取消等待会通知阻塞线程停止获取锁；线程在重试边界及交付前检查状态，迟到租约随结果丢弃而释放。慢速或已开始的文件系统操作不能承诺立即中断。

锁只提供协调，不回滚回调已经完成的文件或状态发布。外部程序、旧实现和不使用同一插件锁的调用不参与协调；需要跨文件恢复时，业务包必须另行设计日志和提交协议。锁文件属于宿主管理数据，不作为用户删除后解除占用的接口。

## 宿主与验证

`PluginHost::plugin_lock(key, timeout_ms, capabilities)` 返回 `Box<dyn PluginLock>`；旧宿主默认返回不可用。正式实现位于 `src/plugins/private/lock.rs`，运行时绑定位于 `crates/sai-plugin-runtime/src/runtime/private/lock.rs`。两层都校验独立存储授权，Lua 无法指定插件标识、状态目录或锁文件路径。

运行时测试覆盖初始化、只读协调、错误参数、嵌套顺序、多返回值、取消和再次调用。正式宿主测试覆盖跨实例竞争、等待超时、取消、128 键限制、稳定 inode、链接边界和独立授权。知识库的读写与后台重建通过此接口组合，业务恢复边界见[知识库插件](knowledge-base.md)。
