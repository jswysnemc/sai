//! 队列管理状态和紧凑视图回归。

use super::queue_panel::*;
mod tests {
    use super::*;
    use crate::agent::AgentMode;
    use crate::cli::repl_clipboard::ReplClipboardState;
    use crate::cli::repl_runtime::{QueueInsertAt, QueuedSubmission};
    use crossterm::event::{KeyCode, KeyModifiers};

    fn item(text: &str) -> QueuedSubmission {
        QueuedSubmission::new(
            AgentMode::Yolo,
            text.to_string(),
            ReplClipboardState::default(),
        )
    }

    #[test]
    fn activate_selects_the_item_nearest_the_input() {
        let mut panel = QueuePanelState::default();
        assert!(!panel.activate(0));
        assert!(panel.activate(3));
        assert_eq!(panel.selected(), 2);
        assert!(panel.is_active());
    }

    #[test]
    fn down_on_last_item_returns_to_input() {
        let mut panel = QueuePanelState::default();
        panel.activate(2);
        assert_eq!(panel.selected(), 1);
        assert_eq!(
            panel.handle_key(KeyCode::Down, KeyModifiers::NONE, 2),
            QueuePanelAction::Exit
        );
        assert!(!panel.is_active());
    }

    #[test]
    fn down_on_a_higher_item_moves_toward_the_input() {
        let mut panel = QueuePanelState::default();
        panel.activate(3);
        assert_eq!(
            panel.handle_key(KeyCode::Up, KeyModifiers::NONE, 3),
            QueuePanelAction::Consumed
        );
        assert_eq!(panel.selected(), 1);
        assert_eq!(
            panel.handle_key(KeyCode::Down, KeyModifiers::NONE, 3),
            QueuePanelAction::Consumed
        );
        assert_eq!(panel.selected(), 2);
        assert!(panel.is_active());
    }

    #[test]
    fn up_stops_at_the_first_item() {
        let mut panel = QueuePanelState::default();
        panel.activate(2);
        panel.selected = 0;
        assert_eq!(
            panel.handle_key(KeyCode::Up, KeyModifiers::NONE, 2),
            QueuePanelAction::Consumed
        );
        assert_eq!(panel.selected(), 0);
    }

    #[test]
    fn enter_edits_and_s_sends() {
        let mut panel = QueuePanelState::default();
        panel.activate(1);
        assert_eq!(
            panel.handle_key(KeyCode::Enter, KeyModifiers::NONE, 1),
            QueuePanelAction::Edit
        );
        assert_eq!(
            panel.handle_key(KeyCode::Char('s'), KeyModifiers::NONE, 1),
            QueuePanelAction::SendNow
        );
        assert_eq!(
            panel.handle_key(KeyCode::Char('d'), KeyModifiers::NONE, 1),
            QueuePanelAction::Consumed
        );
        assert_eq!(
            panel.handle_key(KeyCode::Char('d'), KeyModifiers::NONE, 1),
            QueuePanelAction::Delete
        );
        assert_eq!(
            panel.handle_key(KeyCode::Backspace, KeyModifiers::NONE, 1),
            QueuePanelAction::Consumed
        );
        assert_eq!(
            panel.handle_key(KeyCode::Tab, KeyModifiers::NONE, 1),
            QueuePanelAction::ToggleInsert
        );
    }

    #[test]
    fn idle_preview_mentions_ctrl_up() {
        let panel = QueuePanelState::default();
        let lines = panel.panel_lines(
            &[item("one"), item("two"), item("three"), item("four")],
            false,
        );
        let joined = lines.join("\n");
        assert_eq!(lines.len(), 1);
        assert!(joined.contains("Ctrl+↑") || joined.contains("Ctrl+Up"));
        assert!(joined.contains("(4)"));
        assert!(joined.contains("four"));
        assert!(!joined.contains('❯'));
        assert!(joined.contains("下轮接力") || joined.contains("Next turn"));
    }

    #[test]
    fn idle_preview_labels_request_interval_queue() {
        let panel = QueuePanelState::default();
        let mut request = item("now");
        request.insert_at = QueueInsertAt::Request;
        let joined = panel.panel_lines(&[request], false).join("\n");
        assert!(joined.contains("次回插话") || joined.contains("Steer"));
    }

    #[test]
    fn active_list_highlights_selection() {
        let mut panel = QueuePanelState::default();
        panel.activate(2);
        let lines = panel.panel_lines(&[item("first"), item("second")], false);
        let joined = lines.join("\n");
        assert!(joined.contains('›'));
        assert!(joined.contains("second"));
        assert!(joined.contains("first"));
        assert!(joined.contains("下轮接力") || joined.contains("Next turn"));
        assert!(joined.contains("Tab"));
        assert!(joined.contains("立即发送") || joined.contains("Send now"));
    }

    /// 【TUI】【队列】流式阶段 s 键文案是下次请求，不是立即发送。
    #[test]
    fn streaming_help_says_next_request_instead_of_send_now() {
        let mut panel = QueuePanelState::default();
        panel.activate(1);
        let joined = panel.panel_lines(&[item("queued")], true).join("\n");
        assert!(joined.contains("次回插话") || joined.contains("Steer"));
        assert!(!joined.contains("立即发送") && !joined.contains("Send now"));
    }
}
