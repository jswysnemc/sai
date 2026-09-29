use super::PromptInput;
use crate::i18n::text as t;
use crate::ssh::{InteractiveKind, SecretRequest};

/// 卡片标题色，与 CLI 流式模式的 SSH 标记同色。
const TITLE: &str = "\x1b[1m\x1b[38;5;39m";
/// 主机名等次要信息。
const MUTED: &str = "\x1b[2m";
/// 指纹变更警告色。
const WARN: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// 【SSH 征询】【TUI 卡片】挂在输入框上方的征询卡片与输入框占位提示。
///
/// 只保存不含秘密的展示信息；秘密长度也不展示，只提示“已输入”。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::cli) struct SshPromptView {
    title: String,
    host: String,
    prompt: String,
    fingerprint: Option<String>,
    changed: bool,
    placeholder: String,
}

impl SshPromptView {
    /// 按征询请求与当前输入状态生成视图。
    ///
    /// 参数:
    /// - `request`: 交互征询（不含秘密）
    /// - `input`: 输入状态机
    ///
    /// 返回:
    /// - 卡片视图
    pub(in crate::cli) fn new(request: &SecretRequest, input: &PromptInput) -> Self {
        Self {
            title: kind_title(request.kind).to_string(),
            host: request.host_label.clone(),
            prompt: request.prompt.trim().to_string(),
            fingerprint: request.fingerprint.clone(),
            changed: request.changed,
            placeholder: placeholder(input),
        }
    }

    /// 输入框占位提示（以灰色显示在输入行内）。
    ///
    /// 返回:
    /// - 占位提示文本
    pub(in crate::cli) fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// 生成卡片行，按终端宽度折行。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - ANSI 卡片行
    pub(in crate::cli) fn panel_lines(&self, cols: usize) -> Vec<String> {
        let width = cols.saturating_sub(4).max(8);
        // 1. 标题行：类型 + 主机
        let mut lines = vec![format!(
            "{TITLE}● {}{RESET} {MUTED}·{RESET} {}",
            self.title, self.host
        )];
        // 2. 后端给出的说明，逐段折行并缩进
        for paragraph in self.prompt.lines().filter(|line| !line.trim().is_empty()) {
            push_wrapped(&mut lines, paragraph.trim(), "", width);
        }
        // 3. 主机指纹及变更警告
        if let Some(fingerprint) = &self.fingerprint {
            push_wrapped(&mut lines, &format!("SHA256 {fingerprint}"), MUTED, width);
            if self.changed {
                push_wrapped(
                    &mut lines,
                    t(
                        "Warning: this fingerprint differs from known_hosts",
                        "警告：该主机指纹与 known_hosts 记录不一致",
                    ),
                    WARN,
                    width,
                );
            }
        }
        lines
    }
}

/// 按征询类型返回卡片标题。
///
/// 参数:
/// - `kind`: 征询类型
///
/// 返回:
/// - 标题文本
fn kind_title(kind: InteractiveKind) -> &'static str {
    match kind {
        InteractiveKind::Passphrase => t("SSH key passphrase", "SSH 私钥口令"),
        InteractiveKind::Password => t("SSH login password", "SSH 登录密码"),
        InteractiveKind::SudoPassword => t("Remote sudo password", "远端 sudo 密码"),
        InteractiveKind::HostKey => t("Confirm host key", "确认主机指纹"),
        InteractiveKind::DangerCommand => t("Confirm risky command", "确认高危命令"),
    }
}

/// 按输入状态生成输入框占位提示。
///
/// 参数:
/// - `input`: 输入状态机
///
/// 返回:
/// - 占位提示
fn placeholder(input: &PromptInput) -> String {
    if !input.is_secret() {
        return t(
            "y confirm · n / Enter reject · Esc cancel",
            "y 确认 · n / Enter 拒绝 · Esc 取消",
        )
        .to_string();
    }
    if input.has_input() {
        t(
            "Typed (hidden) · Enter submit · Ctrl+U clear · Esc cancel",
            "已输入（不回显）· Enter 提交 · Ctrl+U 清空 · Esc 取消",
        )
        .to_string()
    } else {
        t(
            "Type the secret (hidden) · Enter submit · Esc cancel",
            "输入内容不回显 · Enter 提交 · Esc 取消",
        )
        .to_string()
    }
}

/// 折行追加一段文本，续行缩进两列与标题正文对齐。
///
/// 参数:
/// - `lines`: 输出行
/// - `text`: 纯文本段落
/// - `style`: 段落样式前缀
/// - `width`: 可用宽度
///
/// 返回:
/// - 无
fn push_wrapped(lines: &mut Vec<String>, text: &str, style: &str, width: usize) {
    for line in crate::render::transcript::AnsiLine::wrap_block(text, width) {
        lines.push(format!("  {style}{}{RESET}", line.as_str()));
    }
}
