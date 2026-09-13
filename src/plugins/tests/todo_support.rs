use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

/// 【待办测试】【公共记录】定位公共宿主创建的固定命名空间，供损坏及字节保留测试使用
/// @param paths 隔离应用目录；session 为可信会话作用域
/// @returns plan 键的实际保存路径
pub(super) fn record_path(paths: &SaiPaths, session: &str) -> std::path::PathBuf {
    let (_, directory) = crate::plugins::private::paths::namespace(
        &paths.state_dir,
        "plugin-state",
        "todo",
        session,
    )
    .unwrap();
    directory.join(format!("{}.json", blake3::hash(b"plan").to_hex()))
}

/// 【待办测试】【固定公共状态】通过正式宿主写入单条测试记录，避免借用旧会话文件
/// @param paths 隔离目录；session 为可信作用域；items 为活动项；history 为归档
/// @returns 无；仅用于构造并发与故障测试前置状态
pub(super) fn seed(paths: &SaiPaths, session: &str, items: Value, history: Value) {
    use sai_plugin_runtime::host::{PluginHost, StorageRequest};
    PrivatePluginHost::new(paths, "todo")
        .storage(
            StorageRequest::Set {
                key: "plan".into(),
                value: json!({"version":1,"items":items,"history":history}),
            },
            session,
            &package().manifest.capabilities,
        )
        .unwrap();
}

/// 【待办测试】【显式导入】通过发布命令接续固定状态，不使用宿主按 ID 读取旧文件
/// @param runtime 待办实例；root 为工作目录；session 为可信会话；value 为完整旧状态
/// @returns 无；导入失败立即终止当前测试
pub(super) async fn import(runtime: &PluginRuntime, root: &Path, session: &str, value: Value) {
    runtime
        .call_command(
            "import",
            &json!({"state":value}).to_string(),
            context(root, session, true),
        )
        .await
        .unwrap();
}

/// 【待办测试】【发布包】取得实际发布的 Lua 源码，缺失包时直接报告迁移未完成
/// @returns 完整待办插件包
pub(super) fn package() -> PluginPackage {
    super::example_support::package("todo")
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

/// 【待办测试】【正式安装宿主】显式安装授权后绑定公共会话存储
/// @param root 隔离应用根目录
/// @returns 应用路径、普通运行时和宿主
pub(super) fn installed(root: &Path) -> (SaiPaths, PluginRuntime, Arc<PrivatePluginHost>) {
    let paths = SaiPaths::for_tests(root);
    super::example_support::install_enabled("todo", &crate::config::AppConfig::default(), &paths);
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
