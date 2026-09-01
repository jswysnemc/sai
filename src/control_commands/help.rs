use super::ControlSurface;
use crate::config::PasteImageKey;
use crate::i18n::text as t;

/// 生成控制命令帮助文本。
///
/// 参数:
/// - `surface`: 命令入口类型
/// - `paste_image_key`: 当前生效的剪贴板粘贴键位，用于键位帮助
///
/// 返回:
/// - 帮助文本
pub fn help_text(surface: ControlSurface, paste_image_key: PasteImageKey) -> String {
    let mut lines = vec![t("Available command groups:", "可用命令组:").to_string()];
    lines.extend(shared_help_lines(surface));
    if surface == ControlSurface::Repl {
        lines.extend(repl_only_help_lines());
        lines.extend(repl_key_help_lines(paste_image_key));
    }
    lines.join("\n")
}

/// 返回共享命令帮助行。
///
/// 返回:
/// - 帮助行列表
fn shared_help_lines(surface: ControlSurface) -> Vec<String> {
    let mut lines = vec![
        t("Session:", "会话:").to_string(),
        format!(
            "  {}  {}",
            command_label(surface, "/new [title]", "/新建 [标题]"),
            t("create and switch to a new session", "新建并切换会话")
        ),
        format!(
            "  {}  {}",
            command_label(surface, "/resume [id]", "/恢复 [id]"),
            t(
                "resume a session interactively or by id",
                "交互选择或按 ID 恢复会话"
            )
        ),
        format!(
            "  {}  {}",
            command_label(surface, "/rename <title>", "/重命名 <标题>"),
            t("rename the current session", "为当前会话命名")
        ),
        format!(
            "  {}  {}",
            command_label(surface, "/compact", "/压缩"),
            t(
                "manually compact old conversation turns",
                "手动压缩旧会话轮次"
            )
        ),
        format!(
            "  {}  {}",
            command_label(surface, "/clear [all]", "/清空 [全部]"),
            t(
                "clear current conversation; all also clears memory",
                "清空当前会话，全部会同时清空记忆"
            )
        ),
        t("Model:", "模型:").to_string(),
        format!(
            "  {}  {}",
            command_label(surface, "/model", "/模型"),
            t(
                "interactively pick a model (gateway lists models)",
                "交互选择模型（网关下列出模型）"
            )
        ),
        format!(
            "  {}  {}",
            command_label(surface, "/model <index>", "/模型 <序号>"),
            t("switch active model by index", "按序号切换当前模型")
        ),
        t("Agent:", "Agent:").to_string(),
        format!(
            "  {}  {}",
            command_label(surface, "/agent", "/代理"),
            t(
                "interactively pick an agent (gateway lists agents)",
                "交互选择 Agent（网关下列出 Agent）"
            )
        ),
        format!(
            "  {}  {}",
            command_label(surface, "/agent <index>", "/代理 <序号>"),
            t("switch active agent by index", "按序号切换当前 Agent")
        ),
        t("Help:", "帮助:").to_string(),
        format!(
            "  {}  {}",
            command_label(surface, "/help", "/帮助"),
            t("show this help", "显示此帮助")
        ),
    ];
    if surface == ControlSurface::Repl {
        lines.push(format!(
            "  /clear memory  {}",
            t(
                "clear assistant memory without deleting the session",
                "仅清空助手记忆，保留当前会话"
            )
        ));
    }
    lines
}

/// 返回当前入口的命令展示文本。
///
/// 参数:
/// - `surface`: 命令入口类型
/// - `english`: 英文命令
/// - `chinese`: 中文命令
///
/// 返回:
/// - 展示文本
fn command_label(surface: ControlSurface, english: &str, chinese: &str) -> String {
    if surface == ControlSurface::Gateway {
        format!("{english} / {chinese}")
    } else {
        english.to_string()
    }
}

/// 返回 REPL 专用命令帮助行。
///
/// 返回:
/// - 帮助行列表
fn repl_only_help_lines() -> Vec<String> {
    vec![
        t("REPL:", "REPL:").to_string(),
        format!(
            "  /context [85] [8k]  {}",
            t(
                "show context usage; optional session compact ratio and reserve",
                "查看上下文占用；可设本会话压缩比例与预留",
            )
        ),
        format!(
            "  /context reset      {}",
            t("restore global compact defaults", "恢复全局压缩默认")
        ),
        format!(
            "  /providers  {}",
            t("switch provider or model", "切换 provider 或模型")
        ),
        format!(
            "  /config     {}",
            t("open configuration UI", "打开配置界面")
        ),
        format!(
            "  /thinking [level]  {}",
            t(
                "choose active provider thinking level",
                "选择当前 provider 的思考等级"
            )
        ),
        format!(
            "  /ps         {}",
            t("manage background tasks", "管理后台任务")
        ),
        format!(
            "  /subagents  {}",
            t(
                "list session subagents and their liveness",
                "列出会话子智能体及存活状态"
            )
        ),
        format!(
            "  /msg [id|index] <text>  {}",
            t(
                "leave a message on an alive subagent",
                "给存活的子智能体留言"
            )
        ),
        format!(
            "  /plan       {}",
            t("switch to read-only planning mode", "切换到只读计划模式")
        ),
        format!(
            "  /audit      {}",
            t(
                "switch to audited workspace sandbox mode",
                "切换到审核工作区沙箱模式"
            )
        ),
        format!(
            "  /yolo       {}",
            t("switch to YOLO mode", "切换到 YOLO 模式")
        ),
        format!(
            "  /auto       {}",
            t(
                "switch to auto-audit mode (LLM + human in parallel)",
                "切换到自动审核模式（LLM 与人工并行）"
            )
        ),
        format!("  /auto-audit {}", t("same as /auto", "与 /auto 相同")),
        format!(
            "  /goal [text]  {}",
            t("create or update a persistent goal", "创建或更新持久目标")
        ),
        format!(
            "  /tree       {}",
            t(
                "browse the session tree and switch branches (or press Left on an empty prompt)",
                "浏览会话树并切换分支（空输入时按左键同样可进入）"
            )
        ),
        format!(
            "  /undo       {}",
            t(
                "remove last turn and restore prompt",
                "撤销上一轮并恢复输入"
            )
        ),
        format!("  /exit       {}", t("leave REPL", "退出 REPL")),
    ]
}

/// 返回 REPL 快捷键帮助行。
///
/// 返回:
/// - 帮助行列表
fn repl_key_help_lines(paste_image_key: PasteImageKey) -> Vec<String> {
    vec![
        t("Keys:", "快捷键:").to_string(),
        format!(
            "  Tab         {}",
            t(
                "complete @/# / slash; while working, queue the message",
                "补全 @/# / 斜杠；工作时把消息排队"
            )
        ),
        format!(
            "  Shift+Tab   {}",
            t(
                "cycle yolo / audit / auto-audit / plan",
                "循环 yolo / audit / auto-audit / plan 权限模式"
            )
        ),
        format!(
            "  Enter       {}",
            t(
                "send when idle; queue while the model is working",
                "空闲时发送；模型工作时排队"
            )
        ),
        format!("  Shift+Enter {}", t("insert newline", "插入换行")),
        format!(
            "  Ctrl+J      {}",
            t(
                "insert newline, same as Shift+Enter",
                "插入换行，与 Shift+Enter 相同"
            )
        ),
        format!(
            "  {:<11} {}",
            paste_image_key_label(paste_image_key),
            t("paste clipboard text or image", "粘贴剪贴板文本或图片")
        ),
        format!("  Ctrl+L      {}", t("clear screen", "清屏")),
        format!(
            "  Ctrl+G      {}",
            t(
                "edit input buffer in $EDITOR",
                "使用 $EDITOR 编辑输入缓冲区"
            )
        ),
        format!(
            "  Up/Down     {}",
            t("browse input history", "切换输入历史")
        ),
        format!(
            "  Esc Esc     {}",
            t("clear current message", "清空当前消息")
        ),
        format!(
            "  Ctrl+C      {}",
            t(
                "stop the current turn while working; press twice when idle to exit",
                "工作时停止本轮；空闲连按两次退出",
            )
        ),
        format!(
            "  Ctrl+C Ctrl+C {}",
            t("exit REPL when idle", "空闲时退出 REPL")
        ),
    ]
}

/// 返回剪贴板粘贴键位的展示名。
///
/// 键位可配置，帮助里必须写出当前生效的那个：Windows 上默认是 Alt+V，
/// 写死 Ctrl+V 会让人照着按却什么也没发生。
///
/// 参数:
/// - `key`: 当前生效的键位
///
/// 返回:
/// - 展示名，如 `Alt+V` 或 `Ctrl+V/Alt+V`
fn paste_image_key_label(key: PasteImageKey) -> &'static str {
    match key {
        PasteImageKey::CtrlV => "Ctrl+V",
        PasteImageKey::AltV => "Alt+V",
        PasteImageKey::Both => "Ctrl+V/Alt+V",
    }
}

#[cfg(test)]
mod tests {
    use super::help_text;
    use super::ControlSurface;
    use crate::config::PasteImageKey;

    /// 【TUI】【帮助】快捷键说明与 REPL 命令覆盖 Shift+Tab、/audit、/goal。
    #[test]
    fn repl_help_covers_mode_cycle_and_recent_commands() {
        let text = help_text(ControlSurface::Repl, PasteImageKey::CtrlV);
        assert!(text.contains("Shift+Tab"));
        assert!(text.contains("auto-audit"));
        assert!(text.contains("/audit"));
        assert!(text.contains("/goal"));
        assert!(text.contains("/rename"));
        assert!(!text.contains("toggle YOLO/PLAN"));
    }

    /// 【TUI】帮助里写出的粘贴键位必须跟着配置走。
    #[test]
    fn repl_help_names_the_configured_paste_key() {
        let text = help_text(ControlSurface::Repl, PasteImageKey::AltV);
        assert!(text.contains("Alt+V"), "帮助应写出 Alt+V: {text}");
        assert!(
            !text.contains("Ctrl+V      "),
            "配置为 Alt+V 时不应再写死 Ctrl+V: {text}"
        );
        assert!(help_text(ControlSurface::Repl, PasteImageKey::Both).contains("Ctrl+V/Alt+V"));
    }
}
