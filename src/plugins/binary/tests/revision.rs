use super::*;
use std::io::{self, Cursor};
use std::sync::atomic::Ordering;

struct InterruptOnce {
    interrupted: bool,
    input: Cursor<Vec<u8>>,
}

impl Read for InterruptOnce {
    /// 【文件修订测试】【中断读取】首次读取返回 Interrupted，随后读取完整输入
    /// @param output 本次可写缓冲
    /// @returns 实际字节数或首次中断错误
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(ErrorKind::Interrupted.into());
        }
        self.input.read(output)
    }
}

/// 【文件修订测试】【完整摘要】中断后重试并接受精确上限，不把中断当成 EOF
/// @returns 无；摘要与独立计算的完整输入一致
#[test]
fn digest_retries_interrupted_reads_and_accepts_an_exact_limit() {
    let bytes = vec![0xff; 1024];
    let mut reader = InterruptOnce {
        interrupted: false,
        input: Cursor::new(bytes.clone()),
    };
    assert_eq!(
        digest(&mut reader, 1024, &AtomicBool::new(false)).unwrap(),
        <[u8; 32]>::from(Sha256::digest(bytes))
    );
}

/// 【文件修订测试】【增长检测】达到比较上限后额外读取一字节，拒绝新增内容
/// @returns 无；不给出截断摘要
#[test]
fn digest_rejects_data_that_grows_beyond_the_comparison_limit() {
    let error = digest(
        &mut Cursor::new(vec![0; 1025]),
        1024,
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("size limit"), "{error:#}");
}

struct CancelRead<'a> {
    cancelled: &'a AtomicBool,
    eof: bool,
}

impl Read for CancelRead<'_> {
    /// 【文件修订测试】【读取中取消】在系统调用返回字节或 EOF 时设置取消标记
    /// @param output 接收单个测试字节的缓冲
    /// @returns 一个字节或 EOF，不应有第二次读取
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        assert!(!self.cancelled.swap(true, Ordering::SeqCst));
        output[0] = 7;
        Ok(usize::from(!self.eof))
    }
}

/// 【文件修订测试】【块与 EOF 取消】每个块及最终摘要边界都拒绝撤销，不再次读取或返回迟到摘要
/// @returns 无；两种取消时机都报告取消错误
#[test]
fn digest_checks_cancellation_after_chunks_and_after_eof() {
    for eof in [true, false] {
        let cancelled = AtomicBool::new(false);
        let error = digest(
            &mut CancelRead {
                cancelled: &cancelled,
                eof,
            },
            1024,
            &cancelled,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("cancelled"), "{error:#}");
    }
}

struct BrokenRead;

impl Read for BrokenRead {
    /// 【文件修订测试】【错误读取】返回真实读取错误，禁止当成摘要不匹配
    /// @param output 未写入的读取缓冲
    /// @returns 固定权限错误
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(ErrorKind::PermissionDenied.into())
    }
}

/// 【文件修订测试】【读取失败】普通 I/O 错误必须保留错误语义
/// @returns 无；不会返回成功摘要或条件冲突
#[test]
fn digest_propagates_file_read_errors() {
    let error = digest(&mut BrokenRead, 1024, &AtomicBool::new(false)).unwrap_err();
    assert_eq!(
        error.downcast_ref::<io::Error>().unwrap().kind(),
        ErrorKind::PermissionDenied
    );
}

/// 【文件修订测试】【已授权对象替换】规划后的末级链接不能把比较读取重定向到其他文件
/// @returns 无；外部文件保持原样，比较明确失败
#[cfg(unix)]
#[test]
fn revision_comparison_rejects_a_symlink_inserted_after_path_authorization() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    let keep = outside.path().join("keep");
    std::fs::write(&file, b"old").unwrap();
    std::fs::write(&keep, b"keep").unwrap();
    let context = sai_plugin_runtime::host::SystemContext {
        workdir: root.path().to_str().unwrap().into(),
        allow_writes: true,
    };
    let capabilities =
        serde_json::from_value(serde_json::json!({"binary":{"write_paths":["."]}})).unwrap();
    let destination = crate::plugins::binary::paths::plan("file", &context, &capabilities)
        .unwrap()
        .existing()
        .unwrap()
        .unwrap();
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(&keep, &file).unwrap();
    let error = matches(
        &destination,
        &BinaryRevision::Sha256(Sha256::digest(b"keep").into()),
        1024,
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("regular file"), "{error:#}");
    assert_eq!(std::fs::read(keep).unwrap(), b"keep");
}
