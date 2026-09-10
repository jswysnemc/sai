mod operations;
mod process;
mod storage;
mod worker;

use super::record::{LegacyRecord, LegacyStatus};
use crate::paths::SaiPaths;

/// 【旧闹钟样本】【隔离记录】创建只供兼容测试使用的旧状态目录。
/// @returns 临时目录、路径及没有工作进程的旧任务
fn fixture() -> (tempfile::TempDir, SaiPaths, LegacyRecord) {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    let record = LegacyRecord {
        id: "alarm-1700000000000-321".into(),
        label: "  旧标签\u{0001}  ".into(),
        time: "30s".into(),
        audio_file: None,
        due_at: 253402300799,
        pid: None,
        status: LegacyStatus::Scheduled,
    };
    save(&paths, std::slice::from_ref(&record));
    (root, paths, record)
}

/// 【旧闹钟样本】【原文件写入】模拟原父进程发布完整任务快照。
/// @param paths 隔离目录；records 为旧任务
/// @returns 无
fn save(paths: &SaiPaths, records: &[LegacyRecord]) {
    std::fs::write(
        paths.state_dir.join("alarms.json"),
        serde_json::to_vec(records).unwrap(),
    )
    .unwrap();
}

/// 【旧闹钟样本】【显式设置】仅写入测试目录，通知权限由各测试明确选择。
/// @param paths 隔离目录；setting 为闹钟插件配置
/// @returns 无
fn configure(paths: &SaiPaths, setting: serde_json::Value) {
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(
        paths.config_dir.join("plugins.jsonc"),
        serde_json::to_vec(&serde_json::json!({"plugins":{"alarm":setting}})).unwrap(),
    )
    .unwrap();
}
