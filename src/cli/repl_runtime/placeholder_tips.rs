use std::sync::atomic::{AtomicUsize, Ordering};

/// 输入框占位提示的轮换位置。
///
/// 按轮次推进而不是按时间轮换：定时切换会让空输入框不断闪烁，
/// 而每轮结束换一条既能覆盖到全部提示，视觉上又是静止的。
static TIP_INDEX: AtomicUsize = AtomicUsize::new(0);

/// 中文提示语。
///
/// 首条固定为输入引导，其余是随版本更新的功能提示。
const TIPS_ZH: &[&str] = &[
    "输入消息…",
    "Shift+Tab 切换 yolo / audit / plan 权限模式",
    "工作时 Tab 或 Enter 把消息排队，Ctrl+↑ 进入队列管理",
    "工作时 Ctrl+Z 撤回排队消息，Ctrl+Y 清空队列",
    "Ctrl+T 折叠计划面板，Ctrl+O 展开思考或 diff",
    "/tree 浏览会话分支，可从任意历史轮次开新支线",
    "/undo 撤销上一轮，回到发送前的状态",
    "/compact 压缩长对话，保留关键信息继续聊",
    "/subagents 查看子代理，/msg 给运行中的子代理留言",
    "/model 切换供应商与模型，/thinking 调整思考等级",
    "/agent 切换 Agent 档案，各自带独立提示词与工具",
    "/goal 设定长期目标，跨轮次保持在上下文里",
    "/context 查看上下文占用、压缩与记忆状态",
    "/config 打开配置界面，管理供应商、工具与 Skills",
    "# 引入 Skills，@ 引入当前目录文件，输入过程中可过滤",
    "! 开头直接执行 shell，例如 !git status",
    "/ps 查看后台命令，长任务会自动转入后台继续跑",
    "PageUp 打开会话浏览面板，回看本轮之前的输出",
    "/resume 切回其它会话，/new 开一个干净的会话",
];

/// 英文提示语。
const TIPS_EN: &[&str] = &[
    "Add a follow-up",
    "Shift+Tab cycles yolo / audit / plan permission modes",
    "While working, Tab or Enter queues a message; Ctrl+↑ manages the queue",
    "While working, Ctrl+Z undoes a queued message and Ctrl+Y clears the queue",
    "Ctrl+T folds the plan panel, Ctrl+O expands reasoning or a diff",
    "/tree browses session branches and forks from any past turn",
    "/undo rolls back the last turn",
    "/compact condenses a long conversation and keeps going",
    "/subagents lists subagents, /msg leaves a note for a running one",
    "/model switches provider and model, /thinking adjusts effort",
    "/agent switches agent profiles, each with its own prompt and tools",
    "/goal sets a long-running goal that stays in context",
    "/context shows context usage, compaction and memory state",
    "/config opens the configuration UI for providers, tools and skills",
    "# inserts a skill, @ inserts a file from the current directory; both filter as you type",
    "Start with ! to run a shell command, e.g. !git status",
    "/ps lists background commands; long tasks keep running there",
    "PageUp opens the transcript pager to review earlier output",
    "/resume switches to another session, /new starts a clean one",
];

/// 返回当前应展示的占位提示。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 当前轮次对应的提示语
pub(super) fn current_tip() -> &'static str {
    let tips = if crate::i18n::is_zh() {
        TIPS_ZH
    } else {
        TIPS_EN
    };
    let index = TIP_INDEX.load(Ordering::Relaxed) % tips.len();
    tips[index]
}

/// 轮次结束后推进到下一条提示。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
pub(super) fn advance_tip() {
    TIP_INDEX.fetch_add(1, Ordering::Relaxed);
}

/// 重置到首条提示。
///
/// 仅供测试保持顺序稳定。
#[cfg(test)]
pub(super) fn reset_tip() {
    TIP_INDEX.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【TUI】【占位提示】验证首条是输入引导，之后逐轮轮换并回环。
    #[test]
    fn tips_start_with_the_input_hint_and_cycle() {
        reset_tip();
        let first = current_tip();
        assert_eq!(first, current_tip(), "same turn must stay static");

        advance_tip();
        assert_ne!(current_tip(), first);

        let len = if crate::i18n::is_zh() {
            TIPS_ZH.len()
        } else {
            TIPS_EN.len()
        };
        for _ in 1..len {
            advance_tip();
        }
        assert_eq!(current_tip(), first, "index must wrap around");
        reset_tip();
    }
}
