use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::host::{FileReadRequest, PluginHost, SystemContext};
use sai_plugin_runtime::Capabilities;
use serde_json::json;

/// 【文件宿主测试】【调用上下文】为真实临时目录生成授权和可信工作目录。
/// @param root 临时目录；paths 为声明的读取路径
/// @returns 真实宿主接受的上下文和能力集合
fn context(root: &std::path::Path, paths: &[&str]) -> (SystemContext, Capabilities) {
    (
        SystemContext {
            workdir: root.to_string_lossy().into_owned(),
            allow_writes: false,
        },
        serde_json::from_value(json!({"system":{"read_paths":paths}})).unwrap(),
    )
}

/// 【文件宿主测试】【读取参数】创建指定字节边界的文本读取请求。
/// @param path 路径；max_bytes 为读取上限；lossy 为非法 UTF-8 处理方式
/// @returns 读取请求
fn request(path: &str, max_bytes: usize, lossy: bool) -> FileReadRequest {
    FileReadRequest {
        path: path.into(),
        max_bytes,
        lossy,
    }
}

/// 【文件宿主测试】【文本边界】保留 Unicode 边界、截断标记和严格解码错误。
#[tokio::test]
async fn file_text_is_bounded_without_splitting_unicode() {
    let root = tempfile::tempdir().unwrap();
    let (ctx, caps) = context(root.path(), &["."]);
    std::fs::write(root.path().join("text"), "中文abc").unwrap();
    for (limit, expected, truncated) in [
        (1, "", true),
        (4, "中", true),
        (6, "中文", true),
        (9, "中文abc", false),
    ] {
        let output = SaiPluginHost
            .read_text(request("text", limit, false), ctx.clone(), caps.clone())
            .await
            .unwrap();
        assert_eq!(output.text, expected);
        assert_eq!(output.truncated, truncated);
    }
    std::fs::write(root.path().join("invalid"), [0xff, 0xff, b'a']).unwrap();
    assert!(SaiPluginHost
        .read_text(request("invalid", 8, false), ctx.clone(), caps.clone())
        .await
        .is_err());
    let output = SaiPluginHost
        .read_text(request("invalid", 4, true), ctx, caps)
        .await
        .unwrap();
    assert!(output.text.len() <= 4);
    assert!(output.truncated);
}

/// 【文件宿主测试】【单文件授权】文件授权不能枚举父目录或读取相邻文件，未授权路径不能伪装为不存在。
#[tokio::test]
async fn file_grants_do_not_authorize_siblings_or_missing_outside_paths() {
    let root = tempfile::tempdir().unwrap();
    let (ctx, caps) = context(root.path(), &["allowed"]);
    std::fs::write(root.path().join("allowed"), "allowed").unwrap();
    std::fs::write(root.path().join("other"), "outside").unwrap();
    assert_eq!(
        SaiPluginHost
            .read_text(request("allowed", 32, false), ctx.clone(), caps.clone())
            .await
            .unwrap()
            .text,
        "allowed"
    );
    assert!(SaiPluginHost
        .read_text(request("other", 32, false), ctx.clone(), caps.clone())
        .await
        .is_err());
    assert!(SaiPluginHost
        .file_info("missing".into(), ctx.clone(), caps.clone())
        .await
        .is_err());
    assert!(SaiPluginHost
        .read_directory(".".into(), 10, ctx, caps)
        .await
        .is_err());
    let (ctx, caps) = context(root.path(), &["missing"]);
    assert!(SaiPluginHost
        .file_info("missing".into(), ctx, caps)
        .await
        .unwrap()
        .is_none());
}

/// 【文件宿主测试】【目录上限】目录截断与缺失文件分别表达，文件属性保持真实类型。
#[tokio::test]
async fn directory_limits_and_missing_metadata_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    let (ctx, caps) = context(root.path(), &["."]);
    for name in ["a", "b", "c"] {
        std::fs::write(root.path().join(name), "value").unwrap();
    }
    let short = SaiPluginHost
        .read_directory(".".into(), 2, ctx.clone(), caps.clone())
        .await
        .unwrap();
    assert_eq!(short.entries.len(), 2);
    assert!(short.truncated);
    let all = SaiPluginHost
        .read_directory(".".into(), 3, ctx.clone(), caps.clone())
        .await
        .unwrap();
    assert_eq!(all.entries.len(), 3);
    assert!(!all.truncated);
    assert!(all
        .entries
        .iter()
        .all(|entry| entry.is_file && !entry.is_dir));
    assert!(SaiPluginHost
        .file_info("missing/child".into(), ctx.clone(), caps.clone())
        .await
        .unwrap()
        .is_none());
    let info = SaiPluginHost
        .file_info("a".into(), ctx, caps)
        .await
        .unwrap()
        .unwrap();
    assert!(info.is_file && !info.is_dir);
    assert_eq!(info.len, 5);
}

/// 【文件宿主测试】【链接越界】允许指向授权内的链接，拒绝越界链接与其不存在的子项。
#[cfg(unix)]
#[tokio::test]
async fn symlinks_cannot_escape_the_granted_directory() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let (ctx, caps) = context(root.path(), &["."]);
    std::fs::write(root.path().join("inside"), "inside").unwrap();
    std::fs::write(outside.path().join("secret"), "outside").unwrap();
    std::os::unix::fs::symlink(root.path().join("inside"), root.path().join("link")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
    assert_eq!(
        SaiPluginHost
            .read_text(request("link", 32, false), ctx.clone(), caps.clone())
            .await
            .unwrap()
            .text,
        "inside"
    );
    for path in ["escape/secret", "escape/missing/child"] {
        assert!(SaiPluginHost
            .file_info(path.into(), ctx.clone(), caps.clone())
            .await
            .is_err());
        assert!(SaiPluginHost
            .read_text(request(path, 32, false), ctx.clone(), caps.clone())
            .await
            .is_err());
    }
    let items = SaiPluginHost
        .read_directory(".".into(), 10, ctx, caps)
        .await
        .unwrap();
    let escape = items
        .entries
        .iter()
        .find(|entry| entry.name == "escape")
        .unwrap();
    assert!(!escape.is_dir && !escape.is_file);
}

/// 【文件宿主测试】【特殊文件】管道没有写入方时也必须立即拒绝，不能占用阻塞读取线程。
#[cfg(unix)]
#[tokio::test]
async fn special_files_are_rejected_without_waiting_for_a_writer() {
    use std::os::unix::ffi::OsStrExt;
    let root = tempfile::tempdir().unwrap();
    let (ctx, caps) = context(root.path(), &["."]);
    let name = std::ffi::CString::new(root.path().join("pipe").as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        SaiPluginHost.read_text(request("pipe", 32, false), ctx, caps),
    )
    .await
    .unwrap();
    assert!(format!("{:#}", result.unwrap_err()).contains("regular file"));
}

/// 【文件宿主测试】【用户根目录】波浪号由宿主展开，不会隐式授予读取 HOME 环境变量的能力。
#[tokio::test]
async fn user_directory_expansion_is_independent_of_environment_grants() {
    let root = tempfile::tempdir().unwrap();
    let (ctx, caps) = context(root.path(), &["~"]);
    let info = SaiPluginHost
        .file_info("~".into(), ctx, caps.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(info.is_dir);
    assert!(SaiPluginHost.environment("HOME", &caps).is_err());
}
