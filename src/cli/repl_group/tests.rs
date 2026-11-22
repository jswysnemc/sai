use super::input::{handle_event, GroupAction};
use super::state::GroupTuiState;
use super::view::render;
use crate::paths::SaiPaths;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::super_agent::SuperAgentService;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::path::Path;

fn test_paths(root: &Path) -> SaiPaths {
    SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("bash/sai.sh"),
        zsh_hook_file: root.join("zsh/sai.zsh"),
        powershell_hook_file: root.join("ps/sai.ps1"),
    }
}

/// 【群组终端测试】【多宽度渲染】验证宽屏三栏与窄屏两栏均能无 panic 正常绘制群组、成员和聊天流。
#[tokio::test]
async fn group_tui_renders_wide_and_narrow_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service
        .create_group("Alpha Team", "Super-Agent test", "/tmp/ws", Some("Leader"))
        .unwrap();
    let _ = service
        .post_user_message(&group.id, "You", "请安排代码审查", None, None, "tui", false)
        .await
        .unwrap();

    let mut state = GroupTuiState::new();
    state.reload(&service, Some(&group.id)).unwrap();

    for size in [(140, 36), (80, 24)] {
        let frame = render(&mut state, size);
        assert_eq!(frame.len(), usize::from(size.1));
        let plain: String = frame
            .iter()
            .map(|line| strip_ansi_for_test(line))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("SUPER-AGENT GROUP"));
        assert!(plain.contains("Alpha Team"));
        assert!(plain.contains("请安排代码审查"));
    }
}

/// 【群组终端测试】【快捷命令与成员提及】验证输入框指令解析与成员 @mention 插入。
#[test]
fn group_tui_input_parses_commands_and_mentions() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service.create_group("Dev", "", "/tmp", None).unwrap();

    let mut state = GroupTuiState::new();
    state.reload(&service, Some(&group.id)).unwrap();
    state.insert_member_mention("backend-dev");
    assert_eq!(state.input, "@backend-dev ");

    state.input = "/spawn reviewer 检查改动".to_string();
    state.cursor = state.input.len();
    let action = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    assert_eq!(
        action,
        GroupAction::SpawnRunner {
            group_id: group.id,
            name: "reviewer".to_string(),
            specialty: "general".to_string(),
            prompt: "检查改动".to_string(),
        }
    );
}
