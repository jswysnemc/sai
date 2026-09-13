use super::super::process;
use super::fixture;

/// 【旧闹钟测试】【完整参数】核对所有业务字段及真实目录，缺失目录不能被视为相等。
/// @returns 无
#[test]
fn legacy_process_identity_requires_every_argument_and_existing_state_directory() {
    let (root, paths, record) = fixture();
    let arguments = vec![
        "sai".into(),
        "__alarm-worker".into(),
        "--id".into(),
        record.id.clone(),
        "--time".into(),
        record.time.clone(),
        "--label".into(),
        record.label.clone(),
        "--state-dir".into(),
        paths.state_dir.display().to_string(),
    ];
    assert!(process::matches(&arguments, &record, &paths.state_dir));
    for index in [1, 2, 3, 4, 5, 6, 7, 8, 9] {
        let mut changed = arguments.clone();
        changed[index] = "different".into();
        assert!(
            !process::matches(&changed, &record, &paths.state_dir),
            "index {index}"
        );
    }
    let mut duplicate = arguments.clone();
    duplicate[8] = "--time".into();
    assert!(!process::matches(&duplicate, &record, &paths.state_dir));
    let missing = root.path().join("missing");
    let mut arguments = arguments;
    arguments[9] = missing.display().to_string();
    assert!(!process::matches(&arguments, &record, &missing));
}

/// 【旧闹钟测试】【稳定句柄】错误命令行不接收信号，明确捕获的子进程通过句柄终止。
/// @returns 无，只操作本测试创建的 sleep 子进程
#[cfg(target_os = "linux")]
#[test]
fn legacy_linux_capture_rejects_wrong_argv_and_handle_terminates_owned_child() {
    struct Child(std::process::Child);
    impl Drop for Child {
        /// 【旧闹钟测试】【子进程回收】失败断言也回收本测试拥有的进程。
        /// @returns 无
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let (_root, paths, mut record) = fixture();
    let mut child = Child(
        std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap(),
    );
    record.pid = Some(child.0.id());
    assert!(process::capture(&record, &paths.state_dir).is_err());
    assert!(child.0.try_wait().unwrap().is_none());
    let handle = process::Handle::open(child.0.id()).unwrap().unwrap();
    handle.terminate().unwrap();
    assert!(!child.0.wait().unwrap().success());
}
