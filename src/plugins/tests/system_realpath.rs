use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::host::{PluginHost, SystemContext};
use sai_plugin_runtime::Capabilities;
use serde_json::json;

/// 【真实路径测试】【授权与目录】仅解析已授权且存在的普通对象，不使用公开输入覆盖工作目录。
/// @returns 无
#[tokio::test]
async fn system_realpath_resolves_files_within_trusted_read_scope() {
    let root = tempfile::tempdir().unwrap();
    let work = root.path().join("work");
    std::fs::create_dir(&work).unwrap();
    std::fs::write(work.join("audio.wav"), b"fixture").unwrap();
    std::fs::write(root.path().join("outside.wav"), b"outside").unwrap();
    let context = SystemContext {
        workdir: work.display().to_string(),
        allow_writes: false,
    };
    let caps: Capabilities =
        serde_json::from_value(json!({"system":{"read_paths":["."]}})).unwrap();
    assert_eq!(
        SaiPluginHost
            .real_path("audio.wav".into(), context.clone(), caps.clone())
            .await
            .unwrap(),
        dunce::canonicalize(work.join("audio.wav"))
            .unwrap()
            .display()
            .to_string()
    );
    assert!(SaiPluginHost
        .real_path("missing.wav".into(), context.clone(), caps.clone())
        .await
        .is_err());
    assert!(SaiPluginHost
        .real_path(
            root.path().join("outside.wav").display().to_string(),
            context.clone(),
            caps.clone()
        )
        .await
        .is_err());
    assert!(SaiPluginHost
        .real_path("audio.wav".into(), context.clone(), Capabilities::default())
        .await
        .is_err());
    assert!(SaiPluginHost
        .real_path(
            "audio.wav".into(),
            SystemContext {
                workdir: "relative".into(),
                ..context
            },
            caps
        )
        .await
        .is_err());
}

/// 【真实路径测试】【链接和特殊文件】返回授权内链接的真实目标，拒绝越界链接及管道。
/// @returns 无
#[cfg(unix)]
#[tokio::test]
async fn system_realpath_rejects_escaping_links_and_special_files() {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    let root = tempfile::tempdir().unwrap();
    let work = root.path().join("work");
    std::fs::create_dir(&work).unwrap();
    std::fs::write(work.join("audio.wav"), b"fixture").unwrap();
    std::fs::write(root.path().join("outside.wav"), b"outside").unwrap();
    symlink("audio.wav", work.join("alias.wav")).unwrap();
    symlink(root.path().join("outside.wav"), work.join("escape.wav")).unwrap();
    let fifo = work.join("fifo.wav");
    let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let context = SystemContext {
        workdir: work.display().to_string(),
        allow_writes: false,
    };
    let caps: Capabilities =
        serde_json::from_value(json!({"system":{"read_paths":["."]}})).unwrap();
    assert_eq!(
        SaiPluginHost
            .real_path("alias.wav".into(), context.clone(), caps.clone())
            .await
            .unwrap(),
        dunce::canonicalize(work.join("audio.wav"))
            .unwrap()
            .display()
            .to_string()
    );
    for path in ["escape.wav", "fifo.wav"] {
        assert!(SaiPluginHost
            .real_path(path.into(), context.clone(), caps.clone())
            .await
            .is_err());
    }
}
