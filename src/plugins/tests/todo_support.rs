use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

/// 【待办测试】【发布包】取得实际发布的 Lua 源码，缺失包时直接报告迁移未完成
/// @returns 完整待办插件包
pub(super) fn package() -> PluginPackage {
    crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "todo")
        .expect("todo must be a bundled Lua plugin")
}

/// 【待办测试】【独立运行时】使用正式私有存储验证会话持久化，不替代业务代码
/// @param root 隔离应用根目录
/// @returns 新建的插件实例
pub(super) fn runtime(root: &Path) -> PluginRuntime {
    let package = package();
    let grants = package.manifest.capabilities.clone();
    PluginRuntime::load(
        package,
        json!({}),
        grants,
        Arc::new(PrivatePluginHost::new(&SaiPaths::for_tests(root), "todo")),
    )
    .unwrap()
}

/// 【待办测试】【正式兼容宿主】通过内置描述符绑定旧会话文件，调用入口与发布程序一致
/// @param root 隔离应用根目录
/// @returns 应用路径、内置运行时和宿主
pub(super) fn bundled(root: &Path) -> (SaiPaths, PluginRuntime, Arc<PrivatePluginHost>) {
    let paths = SaiPaths::for_tests(root);
    let descriptor = crate::plugins::discover(&crate::config::AppConfig::default(), &paths)
        .plugins
        .into_iter()
        .find(|item| item.package.manifest.id == "todo")
        .unwrap();
    let host = Arc::new(PrivatePluginHost::for_descriptor(&paths, &descriptor).unwrap());
    let runtime = PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host.clone(),
    )
    .unwrap();
    (paths, runtime, host)
}

/// 【待办测试】【旧条目】构造包含全部原字段的固定样本
/// @param id 原标识；status 为原状态
/// @returns 不包含动态时间的样本
pub(super) fn item(id: &str, status: &str) -> Value {
    json!({"id":id,"text":format!("item {id}"),"status":status,"created_at":"old-created","updated_at":"old-updated"})
}

/// 【待办测试】【可信归属】绑定独立会话和操作，参数不能修改存储所属会话
/// @param root 工作目录；session 为会话标识；writable 为宿主写入许可
/// @returns 固定调用上下文
pub(super) fn context(root: &Path, session: &str, writable: bool) -> InvocationContext {
    InvocationContext {
        session_id: session.into(),
        storage_session_id: session.into(),
        operation_id: "todo-operation".into(),
        workdir: root.display().to_string(),
        allow_writes: writable,
        ..Default::default()
    }
}

/// 【待办测试】【工具调用】执行真实 Lua 工具并解析公开结果
/// @param runtime 实例；root 为目录；session 为会话；args 为工具参数
/// @returns 成功结果的 JSON 对象
pub(super) async fn call(
    runtime: &PluginRuntime,
    root: &Path,
    session: &str,
    args: Value,
) -> Value {
    let text = runtime
        .call_tool("todo", args, context(root, session, true))
        .await
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// 【待办测试】【界面快照】界面命令沿用完成归档规则，返回当前清单与历史
/// @param runtime 实例；root 为目录；session 为会话
/// @returns 与 Web 既有接口一致的快照
pub(super) async fn snapshot(runtime: &PluginRuntime, root: &Path, session: &str) -> Value {
    let text = runtime
        .call_command("snapshot", "", context(root, session, false))
        .await
        .unwrap();
    serde_json::from_str(&text).unwrap()
}
