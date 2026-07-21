use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// 进程启动时选定的轮询起点，保证每次启动偏移不同。
fn process_tip_seed() -> usize {
    static SEED: OnceLock<usize> = OnceLock::new();
    *SEED.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as usize)
            .unwrap_or(0)
            ^ std::process::id() as usize
    })
}

/// 返回当前应展示的空输入提示。
///
/// 按墙钟约 8 秒轮询下一条，并结合进程种子偏移，使每次启动起点不同。
///
/// 返回:
/// - 纯文本提示（不含 ANSI）
pub(crate) fn current_composer_tip() -> &'static str {
    let tips = if crate::i18n::is_zh() {
        ZH_TIPS
    } else {
        EN_TIPS
    };
    let elapsed_slots = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| (duration.as_secs() / 8) as usize)
        .unwrap_or(0);
    let index = process_tip_seed().wrapping_add(elapsed_slots) % tips.len();
    tips[index]
}

const EN_TIPS: &[&str] = &[
    "Shift+Tab cycles mode · Enter send · Shift+Enter newline",
    "Type / for commands · /model · /auto · /help",
    "Prefix ! to run a local shell command",
    "Ctrl+O opens pager for all folded blocks (←→ switch)",
    "Ctrl+V pastes images or files into the composer",
    "Up / Down recalls previous messages when input is empty",
    "Double Esc clears the current draft",
    "Modes: yolo · audit · auto · plan",
    "Ctrl+C interrupts the current agent turn",
    "Tab queues input while the agent is working",
    "Use /auto-audit for LLM + human parallel review",
];

const ZH_TIPS: &[&str] = &[
    "Shift+Tab 切换模式 · Enter 发送 · Shift+Enter 换行",
    "输入 / 打开命令 · /model · /auto · /help",
    "以 ! 开头可执行本地 shell 命令",
    "Ctrl+O 打开折叠块全文（←→ 切换）",
    "Ctrl+V 可粘贴图片或文件到输入框",
    "输入为空时用 ↑ / ↓ 翻阅历史消息",
    "连按两次 Esc 清空当前草稿",
    "模式：yolo · audit · auto · plan",
    "Ctrl+C 中断当前智能体轮次",
    "智能体工作时按 Tab 将输入加入队列",
    "用 /auto-audit 启用 LLM 与人工并行审核",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tip_list_is_nonempty_and_stable_seed() {
        let a = process_tip_seed();
        let b = process_tip_seed();
        assert_eq!(a, b);
        assert!(!current_composer_tip().is_empty());
        assert!(EN_TIPS.len() >= 5);
        assert_eq!(EN_TIPS.len(), ZH_TIPS.len());
    }

    #[test]
    fn tui_tips_exclude_web_only_features() {
        let joined = EN_TIPS.join("\n");
        assert!(!joined.contains("web UI"));
        assert!(!joined.contains("@ for files"));
        assert!(!joined.contains("lightbox"));
        assert!(joined.contains("Shift+Tab cycles mode") || joined.contains("Prefix !"));
        let zh = ZH_TIPS.join("\n");
        assert!(!zh.contains("Web 用"));
        assert!(!zh.contains("灯箱"));
    }
}
