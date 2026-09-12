use crate::{
    host::{validate_storage_key, PluginHost},
    runtime::{
        budget,
        control::CallControl,
        system::{charge, error},
    },
    Capabilities, ExecutionLimits,
};
use mlua::{Lua, LuaSerdeExt, MultiValue, Table, Value};
use std::sync::{Arc, Mutex};

#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    timeout_ms: Option<u64>,
}

/// 【插件互斥】【嵌套次序】回调只允许递增键，避免同键重入或反序等待
struct OrderLease(Arc<Mutex<Vec<String>>>);
impl Drop for OrderLease {
    /// 【插件互斥】【嵌套退出】无参数；正常、异常和取消统一弹出本层键
    fn drop(&mut self) {
        if let Ok(mut keys) = self.0.lock() {
            keys.pop();
        }
    }
}

/// 【插件互斥】【次序检查】最多嵌套四层，检查成功后才进入宿主
/// @param keys 当前锁栈；key 为待取得的私有键
/// @returns 嵌套守卫，退出时恢复上一层
fn enter(keys: &Arc<Mutex<Vec<String>>>, key: &str) -> mlua::Result<OrderLease> {
    let mut active = keys
        .lock()
        .map_err(|_| mlua::Error::runtime("plugin lock order poisoned"))?;
    if active.len() >= 4
        || active
            .last()
            .is_some_and(|previous| previous.as_str() >= key)
    {
        return Err(mlua::Error::runtime(
            "plugin locks require increasing keys and at most four nested scopes",
        ));
    }
    active.push(key.to_string());
    Ok(OrderLease(keys.clone()))
}

/// 【插件互斥】【作用域绑定】私有锁不授予文件或状态写入，租约只存在于一次子回调中
/// @param lua 虚拟机；api 为接口；host 为宿主；capabilities 为授权；limits 为预算；control 为调用状态
/// @returns 安装结果，不向 Lua 返回锁句柄
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let storage: Table = api.get("storage")?;
    let plugin: Table = storage.get("plugin")?;
    let keys = Arc::new(Mutex::new(Vec::<String>::new()));
    plugin.set(
        "with_lock",
        lua.create_async_function(
            move |lua, (key, callback, options): (Value, Value, Option<Table>)| {
                let (host, capabilities, limits, control, keys) = (
                    host.clone(),
                    capabilities.clone(),
                    limits.clone(),
                    control.clone(),
                    keys.clone(),
                );
                async move {
                    // 1. 【插件互斥】【可信权限】只读回调可以协调读取，但初始化和未授权调用不能创建锁
                    control.private_session(false)?;
                    if !capabilities.system.plugin_storage {
                        return Err(mlua::Error::runtime("plugin storage is not allowed"));
                    }
                    let Value::String(key) = key else {
                        return Err(mlua::Error::runtime("plugin lock key must be a string"));
                    };
                    let key = key.to_str()?.to_string();
                    validate_storage_key(&key).map_err(error)?;
                    let Value::Function(callback): Value = callback else {
                        return Err(mlua::Error::runtime(
                            "plugin lock callback must be a function",
                        ));
                    };
                    let options: Options = options
                        .map(|value| lua.from_value(Value::Table(value)))
                        .transpose()?
                        .unwrap_or_default();
                    let timeout = options.timeout_ms.unwrap_or(10_000);
                    if !(1..=600_000).contains(&timeout) {
                        return Err(mlua::Error::runtime(
                            "plugin lock timeout must be 1-600000 milliseconds",
                        ));
                    }
                    budget::checkpoint(&lua)?;
                    let _order = enter(&keys, &key)?;
                    charge(&control, limits.system_calls)?;
                    // 2. 【插件互斥】【回调租约】等待、执行与异常全部处于同一 Future，不把租约保存到全局变量
                    let _lease = host
                        .plugin_lock(&key, timeout, &capabilities)
                        .await
                        .map_err(error)?;
                    budget::checkpoint(&lua)?;
                    let result = callback.call_async::<MultiValue>(()).await?;
                    budget::checkpoint(&lua)?;
                    Ok(result)
                }
            },
        )?,
    )
}
