use crate::notifications::delivery::{DeliveryControl, NotificationBackend};
use crate::plugins::notification::send_with_backend;
use anyhow::Result;
use sai_plugin_runtime::host::{
    NotificationDelivery, NotificationRequest, SystemContext, MAX_NOTIFICATION_AUDIO_BYTES,
};
use sai_plugin_runtime::Capabilities;
use serde_json::json;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Default)]
struct Backend {
    pending: AtomicBool,
    active: AtomicUsize,
    calls: AtomicUsize,
    delivered: Mutex<Vec<(String, usize, String)>>,
    entered: Notify,
    released: Notify,
}

struct Lease<'a>(&'a Backend);

impl Drop for Lease<'_> {
    /// 【通知宿主测试】【释放观察】记录阻塞投递工作已结束。
    /// @returns 无
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.released.notify_one();
    }
}

impl NotificationBackend for Backend {
    /// 【通知宿主测试】【平台替身】记录快照字节，在取消或截止时间到达时终止等待。
    /// @param request 请求；audio 为授权文件快照；control 为可信取消和时限
    /// @returns 请求对应的投递结果，不操作真实桌面或音频设备
    fn deliver(
        &self,
        request: &NotificationRequest,
        audio: Option<&[u8]>,
        control: &DeliveryControl,
    ) -> Result<NotificationDelivery> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.active.fetch_add(1, Ordering::SeqCst);
        let _lease = Lease(self);
        self.entered.notify_one();
        while self.pending.load(Ordering::SeqCst) {
            control.check()?;
            std::thread::sleep(Duration::from_millis(5));
        }
        control.check()?;
        let bytes = audio.unwrap_or_default();
        self.delivered.lock().unwrap().push((
            request.title.clone(),
            bytes.len(),
            blake3::hash(bytes).to_hex().to_string(),
        ));
        Ok(NotificationDelivery {
            desktop: request.desktop,
            sound: request.sound.is_some(),
        })
    }
}

/// 【通知宿主测试】【授权样本】授予通知与当前测试目录读取能力。
/// @returns 最小本地音频授权
fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system":{"notify":true,"read_paths":["."]}})).unwrap()
}

/// 【通知宿主测试】【请求样本】只要求播放给定本地文件。
/// @param path 音频文件路径
/// @returns 受限投递请求
fn audio_request(path: &str) -> NotificationRequest {
    serde_json::from_value(json!({"title":"test","desktop":false,"sound":{"path":path}})).unwrap()
}

/// 【通知宿主测试】【可信目录】绑定绝对工作目录与写入权限。
/// @param path 测试目录
/// @returns 可信系统上下文
fn context(path: &Path) -> SystemContext {
    SystemContext {
        workdir: path.display().to_string(),
        allow_writes: true,
    }
}

/// 【通知宿主测试】【权限复核】无授权、只读调用和越界音频都在平台投递之前失败。
#[tokio::test]
async fn notification_host_rechecks_grants_write_access_and_file_scope() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("sample.wav"), b"sample").unwrap();
    let backend = Arc::new(Backend::default());
    for grants in [
        Capabilities::default(),
        serde_json::from_value(json!({"notifications":true,"system":{"read_paths":["."]}}))
            .unwrap(),
        serde_json::from_value(json!({"system":{"notify":true}})).unwrap(),
    ] {
        assert!(send_with_backend(
            audio_request("sample.wav"),
            context(root.path()),
            grants,
            backend.clone()
        )
        .await
        .is_err());
    }
    let mut readonly = context(root.path());
    readonly.allow_writes = false;
    assert!(send_with_backend(
        audio_request("sample.wav"),
        readonly,
        capabilities(),
        backend.clone()
    )
    .await
    .is_err());
    let outside = tempfile::NamedTempFile::new().unwrap();
    assert!(send_with_backend(
        audio_request(&outside.path().display().to_string()),
        context(root.path()),
        capabilities(),
        backend.clone()
    )
    .await
    .is_err());
    assert!(send_with_backend(
        audio_request("../outside"),
        context(root.path()),
        capabilities(),
        backend.clone()
    )
    .await
    .is_err());
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        send_with_backend(
            audio_request("sample.wav"),
            context(root.path()),
            capabilities(),
            backend.clone()
        )
        .await
        .unwrap(),
        NotificationDelivery {
            desktop: false,
            sound: true
        }
    );
    assert_eq!(backend.delivered.lock().unwrap()[0].1, 6);
    assert_eq!(
        backend.delivered.lock().unwrap()[0].2,
        blake3::hash(b"sample").to_hex().to_string()
    );
}

/// 【通知宿主测试】【大小与类型】音频读取有固定字节上限，空文件和目录不能启动投递。
#[tokio::test]
async fn notification_audio_is_bounded_and_requires_nonempty_regular_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("audio.wav");
    let backend = Arc::new(Backend::default());
    for size in [0, MAX_NOTIFICATION_AUDIO_BYTES as u64 + 1] {
        std::fs::File::create(&path).unwrap().set_len(size).unwrap();
        assert!(send_with_backend(
            audio_request("audio.wav"),
            context(root.path()),
            capabilities(),
            backend.clone()
        )
        .await
        .is_err());
    }
    assert!(send_with_backend(
        audio_request("."),
        context(root.path()),
        capabilities(),
        backend.clone()
    )
    .await
    .is_err());
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    std::fs::File::create(&path)
        .unwrap()
        .set_len(MAX_NOTIFICATION_AUDIO_BYTES as u64)
        .unwrap();
    send_with_backend(
        audio_request("audio.wav"),
        context(root.path()),
        capabilities(),
        backend.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        backend.delivered.lock().unwrap()[0].1,
        MAX_NOTIFICATION_AUDIO_BYTES
    );
}

/// 【通知宿主测试】【链接与管道】链接不能越过读取授权，FIFO 不会阻塞音频读取。
#[cfg(unix)]
#[tokio::test]
async fn notification_audio_rejects_escaping_symlinks_and_special_files() {
    use std::os::unix::ffi::OsStrExt;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link.wav")).unwrap();
    let fifo = root.path().join("pipe.wav");
    let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let backend = Arc::new(Backend::default());
    for path in ["link.wav", "pipe.wav"] {
        assert!(tokio::time::timeout(
            Duration::from_secs(2),
            send_with_backend(
                audio_request(path),
                context(root.path()),
                capabilities(),
                backend.clone()
            )
        )
        .await
        .unwrap()
        .is_err());
    }
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
}

/// 【通知宿主测试】【内置资源】内置声音不依赖文件读取授权，文件内容仍只交给宿主后端。
#[tokio::test]
async fn builtin_sounds_need_no_filesystem_capability() {
    let root = tempfile::tempdir().unwrap();
    let backend = Arc::new(Backend::default());
    for builtin in ["alarm", "chime"] {
        let request = serde_json::from_value(
            json!({"title":"test","desktop":false,"sound":{"builtin":builtin}}),
        )
        .unwrap();
        let caps = serde_json::from_value(json!({"system":{"notify":true}})).unwrap();
        assert!(
            send_with_backend(request, context(root.path()), caps, backend.clone())
                .await
                .unwrap()
                .sound
        );
    }
    assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
}

/// 【通知宿主测试】【取消释放】超时或丢弃 Future 会停止投递线程，后续调用可以正常完成。
#[tokio::test]
async fn cancelling_or_timing_out_native_delivery_prevents_late_output() {
    for cancel in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let backend = Arc::new(Backend::default());
        backend.pending.store(true, Ordering::SeqCst);
        let request: NotificationRequest = serde_json::from_value(
            json!({"title":"test","timeout_ms":if cancel {2000} else {200}}),
        )
        .unwrap();
        let task = tokio::spawn(send_with_backend(
            request,
            context(root.path()),
            capabilities(),
            backend.clone(),
        ));
        tokio::time::timeout(Duration::from_secs(2), backend.entered.notified())
            .await
            .unwrap();
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            assert!(format!("{:#}", task.await.unwrap().unwrap_err()).contains("timed out"));
        }
        tokio::time::timeout(Duration::from_secs(2), backend.released.notified())
            .await
            .unwrap();
        assert_eq!(backend.active.load(Ordering::SeqCst), 0);
        assert!(backend.delivered.lock().unwrap().is_empty());
        backend.pending.store(false, Ordering::SeqCst);
        let request = serde_json::from_value(json!({"title":"recovered"})).unwrap();
        send_with_backend(
            request,
            context(root.path()),
            capabilities(),
            backend.clone(),
        )
        .await
        .unwrap();
        assert_eq!(backend.delivered.lock().unwrap().len(), 1);
    }
}
