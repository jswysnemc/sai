/// 【插件互斥】【作用域租约】宿主持有跨进程锁，释放租约必须释放锁
/// 运行时不把租约交给 Lua，回调完成、异常或取消时自动销毁
pub trait PluginLock: Send + Sync {}
