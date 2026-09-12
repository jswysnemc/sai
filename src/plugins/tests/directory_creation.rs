use super::binary_conditional_support::{context, with_host};
use crate::{
    paths::SaiPaths,
    plugins::{host::SaiPluginHost, private::PrivatePluginHost},
};
use sai_plugin_runtime::{
    host::{PluginHost, SystemContext},
    Capabilities, PluginRuntime,
};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

const SOURCE: &str = r#"
--- 【目录测试】【调用入口】输入路径和可选伪造标志；返回实际创建目录
local function run(args,ctx)
    if args.forge then ctx.allow_writes=true end
    return sai.fs.create_dir(args.path)
end
sai.register_tool({name='run',description='Directory creation',access='optional_writes',parameters={type='object'},execute=run})
"#;

/// 【目录测试】【真实实例】输入隔离目录和写入根；返回正式文件宿主实例
fn runtime(root: &Path, writes: &[&str]) -> PluginRuntime {
    with_host(
        Arc::new(PrivatePluginHost::new(
            &SaiPaths::for_tests(root),
            "directory-test",
        )),
        &[],
        writes,
        SOURCE,
        |_| {},
    )
}

/// 【目录测试】【根目录和嵌套】目录创建只需写入授权，返回规范路径且重复执行幂等
/// @returns 无；库根和缺失子目录均能创建
#[tokio::test]
async fn directory_creation_is_idempotent_and_accepts_granted_root() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path(), &["output"]);
    for name in ["output", "output/a/b", "output/a/b"] {
        let output = plugin
            .call_tool("run", json!({"path":name}), context(root.path()))
            .await
            .unwrap();
        assert_eq!(
            Path::new(&output),
            dunce::canonicalize(root.path().join(name)).unwrap()
        );
        assert!(root.path().join(name).is_dir());
    }
}

/// 【目录测试】【授权和参数】缺失权限、计划模式、父目录和错误类型都不能创建目录
/// @returns 无；Lua 修改上下文不改变宿主权限
#[tokio::test]
async fn directory_creation_rejects_invalid_grants_paths_and_contexts() {
    let root = tempfile::tempdir().unwrap();
    for (grants, writes, args) in [
        (vec![], true, json!({"path":"output"})),
        (vec!["output"], false, json!({"path":"output","forge":true})),
        (vec!["output"], true, json!({"path":"outside/child"})),
        (vec!["output"], true, json!({"path":"output/../outside"})),
        (vec!["output"], true, json!({"path":1})),
        (vec!["output"], true, json!({"path":null})),
    ] {
        let plugin = runtime(root.path(), &grants);
        let mut ctx = context(root.path());
        ctx.allow_writes = writes;
        assert!(plugin.call_tool("run", args, ctx).await.is_err());
    }
    assert!(!root.path().join("output").exists());
    assert!(!root.path().join("outside").exists());
}

/// 【目录测试】【宿主复核】绕过 Lua 直接调用仍须取得真实写入授权
/// @returns 无；缺失授权及已存在文件都不产生目录
#[tokio::test]
async fn directory_creation_host_rechecks_grants_and_existing_type() {
    let root = tempfile::tempdir().unwrap();
    let caps: Capabilities =
        serde_json::from_value(json!({"binary":{"write_paths":["output"]}})).unwrap();
    let ctx = SystemContext {
        workdir: root.path().display().to_string(),
        allow_writes: true,
    };
    for grants in [Capabilities::default(), caps.clone()] {
        let mut readonly = ctx.clone();
        readonly.allow_writes = false;
        assert!(SaiPluginHost
            .create_directory("output".into(), readonly, grants)
            .await
            .is_err());
    }
    std::fs::write(root.path().join("output"), b"keep").unwrap();
    assert!(SaiPluginHost
        .create_directory("output".into(), ctx, caps)
        .await
        .is_err());
    assert_eq!(std::fs::read(root.path().join("output")).unwrap(), b"keep");
}

/// 【目录测试】【链接归属】初始链接经规范路径校验，越界链接不能创建外部子项
/// @returns 无；已授权范围内的别名可以复用，越界目标保持不变
#[cfg(unix)]
#[tokio::test]
async fn directory_creation_follows_only_links_within_the_granted_root() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("output/inside")).unwrap();
    std::os::unix::fs::symlink(
        root.path().join("output/inside"),
        root.path().join("output/link"),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("output/escape")).unwrap();
    let plugin = runtime(root.path(), &["output"]);
    plugin
        .call_tool(
            "run",
            json!({"path":"output/link/child"}),
            context(root.path()),
        )
        .await
        .unwrap();
    assert!(root.path().join("output/inside/child").is_dir());
    assert!(plugin
        .call_tool(
            "run",
            json!({"path":"output/escape/child"}),
            context(root.path())
        )
        .await
        .is_err());
    assert!(!outside.path().join("child").exists());
}

/// 【目录测试】【修改时间】文件属性使用真实 Unix 秒，未提供时间的测试宿主不输出该字段
/// @returns 无；时间、长度和 JSON 可选字段保持一致
#[tokio::test]
async fn file_info_exposes_modified_seconds_without_changing_optional_shape() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, b"text").unwrap();
    let expected = std::fs::metadata(&path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let caps: Capabilities =
        serde_json::from_value(json!({"system":{"read_paths":["file"]}})).unwrap();
    let info = SaiPluginHost
        .file_info(
            "file".into(),
            SystemContext {
                workdir: root.path().display().to_string(),
                allow_writes: false,
            },
            caps,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(info.modified, Some(expected));
    assert_eq!(info.len, 4);
    let encoded = serde_json::to_value(sai_plugin_runtime::host::FileInfo {
        modified: None,
        ..info
    })
    .unwrap();
    assert_eq!(encoded, json!({"is_file":true,"is_dir":false,"len":4}));
    assert_eq!(encoded.get("modified"), None::<&Value>);
}
