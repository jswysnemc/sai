use super::process::run_shell_command_with_progress;
use crate::tools::ToolProgress;
use std::time::Duration;

/// 【命令】【取消回归】取消前台命令必须终止后代进程并关闭进度管道；无参数、无返回值
#[cfg(unix)]
#[tokio::test]
async fn cancelling_command_stops_descendants_and_output_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("leaked-child");
    let command = format!(
        "(sleep 0.4; printf leaked > '{}') & printf ready; wait",
        marker.display()
    );
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        run_shell_command_with_progress(&command, 30, "/bin/sh", None, ToolProgress::new(sender))
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert!(!marker.exists(), "取消后后代进程仍写入文件");
    while receiver.try_recv().is_ok() {}
    assert!(
        tokio::time::timeout(Duration::from_millis(250), receiver.recv())
            .await
            .unwrap()
            .is_none()
    );
}
