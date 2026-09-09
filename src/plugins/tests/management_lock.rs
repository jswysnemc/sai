use crate::paths::SaiPaths;
use crate::plugins::config::mutation_lock;

/// 【插件锁测试】【作用域释放】同一目录互斥、不同目录独立，配置失败后仍可执行下一条管理命令。
#[test]
fn management_locks_are_scoped_and_reusable_after_errors() {
    let root = tempfile::tempdir().unwrap();
    let other_root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let other = SaiPaths::for_tests(other_root.path());
    let lock = mutation_lock(&paths).unwrap();
    assert!(mutation_lock(&paths).is_err());
    assert!(mutation_lock(&other).is_ok());
    drop(lock);
    let config = crate::config::AppConfig::default();
    for enabled in [false, true] {
        assert!(
            crate::plugins::configure(&config, &paths, "web-search", serde_json::json!([]))
                .is_err()
        );
        crate::plugins::set_enabled(
            &config,
            &paths,
            "web-search",
            enabled,
            crate::plugins::GrantUpdate::Keep,
        )
        .unwrap();
    }
}

/// 【插件锁测试】【进程隔离】在独立测试进程中暂停 fork 后的子进程，避免继承并行测试的描述符。
#[cfg(unix)]
#[test]
fn inherited_descriptor_does_not_keep_a_finished_management_lock() {
    const MARKER: &str = "SAI_PLUGIN_LOCK_INHERITANCE_FIXTURE";
    if std::env::var_os(MARKER).is_some() {
        reproduce_inherited_lock();
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "plugins::tests::management_lock::inherited_descriptor_does_not_keep_a_finished_management_lock",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MARKER, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 【插件锁测试】【继承重现】让真实子进程在 exec 之前等待，管理调用结束后应立即允许下次调用。
/// @returns 无；失败前先释放子进程，防止测试遗留进程
#[cfg(unix)]
fn reproduce_inherited_lock() {
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt;
    use std::time::Duration;

    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let lock = mutation_lock(&paths).unwrap();
    assert!(
        mutation_lock(&paths).is_err(),
        "active management calls must remain exclusive"
    );
    let (mut parent_gate, child_gate) = UnixStream::pair().unwrap();
    parent_gate
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let descriptor = child_gate.as_raw_fd();
    let mut command = std::process::Command::new("/bin/sh");
    command.args(["-c", "exit 0"]);

    // 【插件锁测试】【继承重现】1. fork 后只使用异步信号安全的系统调用，保留继承的锁描述符直到父进程检查完成
    unsafe {
        command.pre_exec(move || {
            let byte = [1u8];
            if libc::write(descriptor, byte.as_ptr().cast(), 1) != 1 {
                return Err(std::io::Error::last_os_error());
            }
            let mut resume = [0u8];
            if libc::read(descriptor, resume.as_mut_ptr().cast(), 1) != 1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = std::thread::spawn(move || {
        let spawned = command.spawn();
        drop(child_gate);
        spawned.and_then(|mut child| child.wait())
    });
    let mut ready = [0u8];
    let handshake = parent_gate.read_exact(&mut ready);

    // 【插件锁测试】【继承重现】2. 只结束父调用，子进程仍处于执行前等待状态
    drop(lock);
    let next = mutation_lock(&paths);
    let resumed = parent_gate.write_all(&[1]);
    let completed = child.join().unwrap();
    handshake.unwrap();
    resumed.unwrap();
    assert!(completed.unwrap().success());
    assert!(
        next.is_ok(),
        "finished management call retained by a child descriptor: {:?}",
        next.err()
    );
}
