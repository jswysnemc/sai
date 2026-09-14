use super::session_resume_choices;
use crate::paths::SaiPaths;
use crate::state::{session_index_io_counts, SessionInfo};

/// 【会话载入】【列表规模】终端选择器读取一次工作区索引，正常展示不重写索引。
/// @returns 无；无参数，失败时报告实际磁盘访问次数
#[tokio::test]
async fn resume_choices_have_bounded_index_io() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    crate::runtime_cwd::scope(workspace, async {
        crate::state::create_session(&paths, None).unwrap();
        let (scope, _) = crate::state::locate_session_dirs(&paths, "default").unwrap();
        let sessions = (0..64)
            .map(|index| SessionInfo {
                id: if index == 0 {
                    "default".into()
                } else {
                    format!("session-{index}")
                },
                title: format!("Session {index}"),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            })
            .collect::<Vec<_>>();
        let original = serde_json::to_vec(&sessions).unwrap();
        std::fs::write(scope.join("index.json"), &original).unwrap();
        std::fs::write(scope.join("current"), "session-63\n").unwrap();

        // 1. 【会话载入】【真实入口】计数覆盖正式选择器，而不是模拟索引调用
        let before = session_index_io_counts();
        let choices = session_resume_choices(&paths).unwrap();
        let after = session_index_io_counts();
        assert_eq!(choices.len(), 64);
        assert!(choices
            .iter()
            .any(|(id, label)| id == "session-63" && label.starts_with('*')));
        assert!(
            after.0 - before.0 <= 3,
            "listing read the index {} times",
            after.0 - before.0
        );
        assert_eq!(after.1 - before.1, 0, "listing must not write the index");
        assert_eq!(std::fs::read(scope.join("index.json")).unwrap(), original);
    })
    .await;
}
