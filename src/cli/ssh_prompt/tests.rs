use super::*;
use crossterm::event::{KeyEvent, KeyEventState};

/// 构造测试用征询请求。
///
/// 参数:
/// - `kind`: 征询类型
///
/// 返回:
/// - 征询请求
fn request(kind: InteractiveKind) -> SecretRequest {
    SecretRequest {
        id: "req".into(),
        session_id: "session".into(),
        kind,
        host_label: "txy".into(),
        prompt: "公钥认证失败，请输入该主机的登录密码。".into(),
        fingerprint: None,
        changed: false,
    }
}

/// 构造按下事件。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键
///
/// 返回:
/// - 终端事件
fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    })
}

/// 键入、退格、粘贴后回车提交完整秘密。
#[test]
fn secret_input_collects_typed_and_pasted_text() {
    let mut input = PromptInput::new(&request(InteractiveKind::Password));
    for ch in "abx".chars() {
        assert_eq!(
            input.handle(key(KeyCode::Char(ch), KeyModifiers::NONE)),
            PromptStep::Continue
        );
    }
    input.handle(key(KeyCode::Backspace, KeyModifiers::NONE));
    input.handle(Event::Paste("c\nd".into()));
    assert!(input.has_input());
    assert_eq!(
        input.handle(key(KeyCode::Enter, KeyModifiers::NONE)),
        PromptStep::Done(SecretResponse::Provided("abcd".into()))
    );
    assert!(!input.has_input());
}

/// Ctrl+U 清空、Esc 与 Ctrl+C 取消，取消后不残留秘密。
#[test]
fn secret_input_clears_and_cancels() {
    let mut input = PromptInput::new(&request(InteractiveKind::SudoPassword));
    input.handle(key(KeyCode::Char('x'), KeyModifiers::NONE));
    input.handle(key(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert!(!input.has_input());
    input.handle(key(KeyCode::Char('y'), KeyModifiers::NONE));
    assert_eq!(
        input.handle(key(KeyCode::Esc, KeyModifiers::NONE)),
        PromptStep::Done(SecretResponse::Cancelled)
    );
    assert!(!input.has_input());
    let mut input = PromptInput::new(&request(InteractiveKind::Passphrase));
    assert_eq!(
        input.handle(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        PromptStep::Done(SecretResponse::Cancelled)
    );
}

/// 确认类默认拒绝，只有 y 才确认。
#[test]
fn confirmation_defaults_to_reject() {
    let mut input = PromptInput::new(&request(InteractiveKind::HostKey));
    assert!(!input.is_secret());
    assert_eq!(
        input.handle(key(KeyCode::Char('x'), KeyModifiers::NONE)),
        PromptStep::Continue
    );
    assert_eq!(
        input.handle(key(KeyCode::Enter, KeyModifiers::NONE)),
        PromptStep::Done(SecretResponse::Confirmed(false))
    );
    assert_eq!(
        input.handle(key(KeyCode::Char('Y'), KeyModifiers::NONE)),
        PromptStep::Done(SecretResponse::Confirmed(true))
    );
}

/// 卡片与占位提示不含秘密，也不暴露秘密长度。
#[test]
fn card_never_reveals_secret_or_length() {
    let request = request(InteractiveKind::Password);
    let mut input = PromptInput::new(&request);
    let empty = SshPromptView::new(&request, &input);
    for ch in "hunter2".chars() {
        input.handle(key(KeyCode::Char(ch), KeyModifiers::NONE));
    }
    let typed = SshPromptView::new(&request, &input);
    let rendered = typed.panel_lines(80).join("\n") + typed.placeholder();
    assert!(!rendered.contains("hunter2"));
    assert!(!rendered.contains('*') && !rendered.contains('•'));
    // 输入前后占位提示变化，但卡片正文一致
    assert_ne!(empty.placeholder(), typed.placeholder());
    assert_eq!(empty.panel_lines(80), typed.panel_lines(80));
    assert!(rendered.contains("txy"));
}

/// 窄终端下长说明折行，每行不超过终端宽度；指纹变更时追加警告。
#[test]
fn card_wraps_and_warns_on_changed_fingerprint() {
    let mut request = request(InteractiveKind::HostKey);
    request.fingerprint = Some("abcdefghijklmnopqrstuvwxyz0123456789".into());
    request.changed = true;
    let input = PromptInput::new(&request);
    let lines = SshPromptView::new(&request, &input).panel_lines(24);
    assert!(lines.len() >= 4);
    for line in &lines {
        assert!(crate::cli::repl_text::visible_width(line) <= 24, "{line}");
    }
    let plain = crate::cli::repl_text::strip_terminal_control_sequences(&lines.concat());
    assert!(plain.replace("  ", "").contains("known_hosts"), "{plain}");
}
