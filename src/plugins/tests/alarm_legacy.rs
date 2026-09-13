use crate::{
    paths::SaiPaths,
    plugins::{legacy_alarm_jobs, scheduler},
};
use serde_json::json;

/// 【闹钟兼容测试】【旧记录查询】既有无进程记录仍可查询，读取不能改写旧工作进程共享的文件。
/// @returns 无，普通调度不暴露旧记录，显式旧任务管理仍可读取
#[test]
fn alarm_plugin_cannot_observe_legacy_records_through_public_scheduler() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    let content = serde_json::to_vec(&json!([{
        "id":"alarm-1700000000000-321", "time":"30s", "label":"已有提醒",
        "audio_file":null, "due_at":253402300799i64, "pid":null, "status":"scheduled"
    }]))
    .unwrap();
    let file = paths.state_dir.join("alarms.json");
    std::fs::write(&file, &content).unwrap();
    assert!(scheduler::list(&paths, "alarm").unwrap().is_empty());
    let tasks = legacy_alarm_jobs::list(&paths).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].command, "deliver");
    let payload: serde_json::Value = serde_json::from_str(&tasks[0].arguments).unwrap();
    assert_eq!(payload["legacy_id"], "alarm-1700000000000-321");
    assert_eq!(std::fs::read(file).unwrap(), content);
}
