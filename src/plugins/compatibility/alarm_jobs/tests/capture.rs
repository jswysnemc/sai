use super::{verify, Handle};
use crate::plugins::compatibility::alarm_jobs::record::{LegacyRecord, LegacyStatus};

/// 【旧闹钟测试】【启动参数竞态】参数暂时缺失时，存活进程不能被报告为已经退出。
/// @returns 无，只检查并回收本测试拥有的子进程
#[test]
fn missing_arguments_never_confirm_exit_of_a_live_process() {
    struct Child(std::process::Child);
    impl Drop for Child {
        /// 【旧闹钟测试】【清理】异常断言也回收本测试拥有的进程。
        /// @returns 无
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut child = Child(
        std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap(),
    );
    let record = LegacyRecord {
        id: "alarm-1700000000000-1".into(),
        label: String::new(),
        time: "30s".into(),
        audio_file: None,
        due_at: 1,
        pid: Some(child.0.id()),
        status: LegacyStatus::Scheduled,
    };
    let handle = Handle::open(child.0.id()).unwrap().unwrap();
    let result = verify(handle, child.0.id(), &record, root.path(), |_| Ok(None));
    assert!(result.is_err());
    assert!(child.0.try_wait().unwrap().is_none());
    let handle = Handle::open(child.0.id()).unwrap().unwrap();
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    assert!(
        verify(handle, record.pid.unwrap(), &record, root.path(), |_| Ok(
            None
        ))
        .unwrap()
        .is_none()
    );
}
