use super::input::{handle_event, GroupAction};
use super::state::{GroupModal, GroupTuiState, InspectorTab};
use super::text_util::{clean_preview_text, wrap_plain};
use super::view::render;
use crate::cli::repl_text::visible_width;
use crate::config::{AppConfig, InputStyle};
use crate::paths::SaiPaths;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::super_agent::SuperAgentService;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::path::Path;

fn test_paths(root: &Path) -> SaiPaths {
    SaiPaths::for_tests(root)
}

/// 【群组终端测试】【原生风格与宽窄屏渲染】验证宽屏两栏与窄屏单栏使用 Sai 原生输入条、展示模型基础信息、无外框输入框与盲文散点。
#[tokio::test]
async fn group_tui_renders_native_style_wide_and_narrow_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let mut cfg = AppConfig::default();
    if let Some(provider) = cfg.providers.first_mut() {
        provider.default_model = "vendor/claude-sonnet-4".to_string();
        provider.thinking_level = "high".to_string();
    }
    cfg.save(&paths).unwrap();

    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service
        .create_group("Alpha Team", "Super-Agent test", "/tmp/ws", Some("Leader"))
        .unwrap();
    let _ = service
        .post_user_message(
            &group.id,
            "You",
            "请安排代码审查并检查 `src/main.rs`",
            None,
            None,
            "tui",
            false,
        )
        .await
        .unwrap();

    let mut state = GroupTuiState::new();
    state.reload(&service, Some(&group.id)).unwrap();
    state.leader_live.thinking = true;
    state.leader_live.activity = "调用工具：创建执行器".to_string();

    for size in [(140, 36), (84, 24)] {
        let frame = render(&mut state, size);
        assert_eq!(frame.lines.len(), usize::from(size.1));
        assert!(frame.cursor.is_some(), "主输入区必须提供真实硬件光标坐标");
        let plain: String = frame
            .lines
            .iter()
            .map(|line| strip_ansi_for_test(line))
            .collect::<Vec<_>>()
            .join("\n");
        // 1. 保留核心群组信息、消息内容与实时活动
        assert!(plain.contains("Alpha Team"), "{plain}");
        assert!(plain.contains("请安排代码审查"), "{plain}");
        assert!(plain.contains("调用工具：创建执行器"), "{plain}");
        // 2. 宽窄屏底栏均清晰展示模型短名与上下文窗口
        assert!(plain.contains("claude-sonnet-4"), "{plain}");
        assert!(plain.contains("group"), "{plain}");
        // 3. 使用 Sai 原生输入提示符 `→`，不再绘制割裂的圆角输入框或盲文散点
        assert!(plain.contains(" → "), "{plain}");
        assert!(!plain.contains("发言 ·"), "{plain}");
        assert!(!plain.contains("⠋") && !plain.contains("⠙"), "{plain}");
        // 4. 每行视觉宽度不得超出终端列数
        for line in &frame.lines {
            assert!(
                visible_width(line) <= usize::from(size.0),
                "width overflow at {:?}: {}",
                size,
                strip_ansi_for_test(line)
            );
        }
    }

    // 宽屏侧栏应同时呈现 CTX 容量指标、Leader/Worker 模型摘要与群组成员
    let wide_plain: String = render(&mut state, (140, 36))
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(wide_plain.contains("CTX"), "{wide_plain}");
    assert!(wide_plain.contains("high"), "{wide_plain}");
    assert!(wide_plain.contains("协作群组"), "{wide_plain}");
    assert!(wide_plain.contains("群组成员"), "{wide_plain}");
    assert!(wide_plain.contains("╭──╴"), "宽屏应渲染 Sai 原生品牌头: {wide_plain}");

    // 窄屏第 0 行不预留侧栏开关空位，进度指示贴齐右缘（仅 1 格内边距）
    let narrow_frame = render(&mut state, (84, 24));
    let narrow_row0 = strip_ansi_for_test(&narrow_frame.lines[0]);
    assert!(
        narrow_row0.ends_with("100% "),
        "窄屏顶栏右侧不得留出无用空白: {narrow_row0:?}"
    );
}

/// 【群组终端测试】【异构模型与输入样式】验证 Leader/Worker 独立模型展示及 `InputStyle::Separator` 原生样式切换。
#[test]
fn group_tui_shows_distinct_worker_model_and_respects_input_style() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let mut cfg = AppConfig::default();
    let provider_id = cfg.providers[0].id.clone();
    cfg.providers[0].models = vec!["opus-leader".to_string(), "flash-worker".to_string()];
    cfg.providers[0].default_model = "opus-leader".to_string();
    cfg.providers[0].thinking_level = "xhigh".to_string();
    cfg.super_agent.worker_provider_id = provider_id;
    cfg.super_agent.worker_model = "flash-worker".to_string();
    cfg.display.input_style = InputStyle::Separator;
    cfg.save(&paths).unwrap();

    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service.create_group("Model Group", "", "/tmp", None).unwrap();
    let mut state = GroupTuiState::new();
    state.reload(&service, Some(&group.id)).unwrap();

    assert!(!state.model_info.worker_shares_leader);
    assert_eq!(state.model_info.leader.short_model, "opus-leader");
    assert_eq!(state.model_info.worker.short_model, "flash-worker");
    assert_eq!(state.model_info.input_style, InputStyle::Separator);

    let frame = render(&mut state, (140, 36));
    let plain: String = frame
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plain.contains("opus-leader"), "{plain}");
    assert!(plain.contains("flash-worker"), "{plain}");
    assert!(plain.contains("xhigh"), "{plain}");
    // Separator 样式在输入区不铺暗色 235 背景条
    let composer_top = usize::from(state.hit_map.composer_row_range.0);
    assert!(!frame.lines[composer_top].contains("\x1b[48;5;235m"));
}

/// 【群组终端测试】【检视页与帮助弹窗】验证渠道页微信引导、工具页边界清单、帮助弹窗模型展示及光标切换。
#[test]
fn group_tui_renders_channels_tools_and_help_modal() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service.create_group("Ops", "", "/tmp", None).unwrap();

    let mut state = GroupTuiState::new();
    state.reload(&service, Some(&group.id)).unwrap();

    // 1. 渠道页包含微信扫码登录与启动命令引导
    state.inspector_tab = InspectorTab::ChannelsGateways;
    let ch_plain = render(&mut state, (140, 36))
        .lines
        .iter()
        .map(|l| strip_ansi_for_test(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(ch_plain.contains("微信 iLink"));
    assert!(ch_plain.contains("weixin-login"));

    // 2. 工具页直接展示 Leader 与 Worker 暴露的工具名称
    state.inspector_tab = InspectorTab::ToolsDesign;
    let tools_plain = render(&mut state, (140, 36))
        .lines
        .iter()
        .map(|l| strip_ansi_for_test(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(tools_plain.contains("super_list_state"));
    assert!(tools_plain.contains("super_spawn_runner"));

    // 3. 帮助弹窗居中于左栏，保留右侧栏完整，隐藏输入光标，按 Esc 关闭
    state.modal = Some(GroupModal::Help);
    let help_frame = render(&mut state, (132, 34));
    assert!(help_frame.cursor.is_none());
    let help_plain = help_frame
        .lines
        .iter()
        .map(|l| strip_ansi_for_test(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(help_plain.contains("群组工作台帮助"));
    assert!(help_plain.contains("/model"));
    assert!(help_plain.contains("CTX"), "宽屏弹窗不得遮挡右侧栏");
    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    assert!(state.modal.is_none());
}

/// 【群组终端测试】【中文字符光标、多行换行与补全交互】验证字符级光标不越界、Shift+Enter 换行、透明内联补全及 `/model` 解析。
#[test]
fn group_tui_input_handles_cjk_editing_multiline_completions_and_model_command() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service.create_group("Dev", "", "/tmp", None).unwrap();

    let mut state = GroupTuiState::new();
    state.reload(&service, Some(&group.id)).unwrap();

    // 1. 中文输入下执行 Ctrl+A / Ctrl+E / Ctrl+W / Delete，验证字符下标安全无 panic
    state.insert_str("你好 世界");
    assert_eq!(state.cursor, 5);
    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)),
    );
    assert_eq!(state.input, "你好 ");
    assert_eq!(state.cursor, 3);
    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
    );
    assert_eq!(state.cursor, 0);
    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)),
    );
    assert_eq!(state.input, "好 ");

    // 2. Shift+Enter 插入多行换行，渲染不溢出且光标行数同步推进
    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
    );
    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
    );
    state.insert_str("第二行");
    assert_eq!(state.input, "好 \n第二行");
    let frame = render(&mut state, (120, 32));
    assert!(frame.cursor.is_some());

    // 3. 输入 `/` 弹出透明内联命令补全，Esc 先收起补全但不丢失输入，Tab 可补全 `/model`
    state.input = "/mo".to_string();
    state.cursor = state.input.chars().count();
    state.completions_dismissed = false;
    assert!(state.completion_items().iter().any(|c| c.label == "/model"));
    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    assert!(state.completions_dismissed);
    assert_eq!(state.input, "/mo");

    state.completions_dismissed = false;
    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
    );
    assert_eq!(state.input, "/model");
    // 完整匹配独立命令后自动收起补全面板，避免遮挡底栏
    assert!(state.completion_items().is_empty());
    let action = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    assert_eq!(action, GroupAction::SelectModel);

    // 4. 点击底栏模型区域同样触发 SelectModel，点击顶栏 `◫` 切换右侧栏开关
    let _ = render(&mut state, (130, 34));
    let (frow, fstart, _fend) = state
        .hit_map
        .footer_model_hit
        .expect("bottom bar must expose clickable model range");
    let click_model = handle_event(
        &mut state,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: fstart,
            row: frow,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert_eq!(click_model, GroupAction::SelectModel);

    let (tstart, _) = state
        .hit_map
        .header_sidebar_toggle
        .expect("wide screen must expose top-right sidebar toggle");
    assert!(state.inspector_open);
    let _ = handle_event(
        &mut state,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: tstart,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert!(!state.inspector_open);

    // 5. `@` 成员补全与 `/spawn` `/bind` `/gateway` 指令解析保持完整
    state.input = "@le".to_string();
    state.cursor = state.input.chars().count();
    state.completions_dismissed = false;
    assert!(state
        .completion_items()
        .iter()
        .any(|c| c.label.eq_ignore_ascii_case("@Leader")));

    state.input = "/spawn reviewer 检查改动".to_string();
    state.cursor = state.input.chars().count();
    let spawn = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    );
    assert_eq!(
        spawn,
        GroupAction::SpawnRunner {
            group_id: group.id.clone(),
            name: "reviewer".to_string(),
            specialty: "general".to_string(),
            prompt: "检查改动".to_string(),
        }
    );

    state.input = "/bind weixin * auto".to_string();
    state.cursor = state.input.chars().count();
    assert_eq!(
        handle_event(
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        ),
        GroupAction::BindChannel {
            group_id: group.id,
            channel: "weixin".to_string(),
            chat_id: "*".to_string(),
            admission: "auto".to_string(),
        }
    );

    state.input = "/gateway start weixin".to_string();
    state.cursor = state.input.chars().count();
    assert_eq!(
        handle_event(
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        ),
        GroupAction::GatewayControl {
            action: "start".to_string(),
            target: "weixin".to_string(),
        }
    );

    // 6. 点击侧栏 Leader 模型行同样触发模型选择器；Ctrl+L 触发全屏重绘；Tab 清理瞬态提示
    state.inspector_open = true;
    state.set_notice("已清空输入；再按一次 Esc 退出群组工作台");
    let _ = render(&mut state, (130, 34));
    let model_row = state
        .hit_map
        .sidebar_model_row
        .expect("sidebar must expose clickable Leader model row");
    let sidebar_col = state.hit_map.sidebar_start_col + 2;
    let sidebar_click = handle_event(
        &mut state,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: sidebar_col,
            row: model_row,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert_eq!(sidebar_click, GroupAction::SelectModel);
    assert!(state.notice.is_none(), "交互动作应清理过期的瞬态提示");

    assert_eq!(
        handle_event(
            &mut state,
            Event::Key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
        ),
        GroupAction::Redraw
    );
}

/// 【群组终端测试】【排版避头点与表单光标】验证中文折行不出现孤立行首标点、预览清洗去井号及表单弹窗真实光标。
#[test]
fn group_tui_wraps_cjk_punctuation_cleanly_and_positions_form_modal_cursor() {
    // 1. 中文避头标点与右括号不得孤立落在折行行首，且每行严格不超出宽度限制
    let samples = [
        ("异步创建隔离上下文(可选 worktree)的 Runner 并绑定任务", 30usize),
        ("群组已就绪，直接发送消息由 Leader 协调任务，或使用 @成员名 交流。", 34usize),
    ];
    for (text, limit) in samples {
        let lines = wrap_plain(text, limit);
        assert!(lines.len() >= 2);
        for line in &lines {
            assert!(visible_width(line) <= limit, "overflow: {line:?}");
            let first = line.chars().next().unwrap_or(' ');
            assert!(
                !matches!(first, '，' | '。' | ')' | '）' | '、'),
                "forbidden line-start punctuation in {lines:?}"
            );
        }
    }
    let worktree_lines = wrap_plain("异步创建隔离上下文(可选 worktree)的 Runner 并绑定任务", 32);
    assert!(
        worktree_lines.iter().any(|line| line.contains("worktree)")),
        "ASCII 单词与右括号应整词携带至下一行，不得拆成 worktre + e): {worktree_lines:?}"
    );
    assert_eq!(
        clean_preview_text("## 最终交付\nRunner\u{3000}已完成"),
        "最终交付 Runner 已完成"
    );

    // 2. Ctrl+R 表单弹窗在宽屏保持右侧栏可见并返回激活字段内的真实硬件光标
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service.create_group("FormTest", "", "/tmp", None).unwrap();
    let mut state = GroupTuiState::new();
    state.reload(&service, Some(&group.id)).unwrap();

    let _ = handle_event(
        &mut state,
        Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
    );
    assert!(matches!(state.modal, Some(GroupModal::SpawnRunner { .. })));
    let frame = render(&mut state, (132, 34));
    let (cur_col, cur_row) = frame.cursor.expect("form modal must show hardware cursor");
    assert!(usize::from(cur_col) < usize::from(state.hit_map.sidebar_start_col));
    assert!(usize::from(cur_row) > 4 && usize::from(cur_row) < 30);
    let plain = frame
        .lines
        .iter()
        .map(|l| strip_ansi_for_test(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plain.contains("启动异步 Runner"), "{plain}");
    assert!(plain.contains("CTX"), "表单弹窗不得截断右侧栏: {plain}");
}
