//! 全屏侧栏与代码审查区的中英双语词条集中定义，英文参数保持原字面量不变以兼容英文界面与既有契约。

/// 【侧栏国际化】【作用域取词】遵循全屏与内联统一展示语言作用域返回双语词条。
/// 参数: en 为英文文案，zh 为中文文案；返回: 当前展示作用域下的静态文本。
fn t(en: &'static str, zh: &'static str) -> &'static str {
    crate::render::tui_presentation::with_locale(|| crate::i18n::text(en, zh))
}

/// 【侧栏国际化】【子智能体标题】返回子智能体分组标题。
/// 参数: 无；返回: 当前语言下的标题文本。
pub(super) fn subagents_heading() -> &'static str {
    t("Subagents", "子智能体")
}

/// 【侧栏国际化】【待办标题】返回待办分组标题。
/// 参数: 无；返回: 当前语言下的标题文本。
pub(super) fn todo_heading() -> &'static str {
    t("Todo", "待办")
}

/// 【侧栏国际化】【变更标题】返回 Git 变更分组标题。
/// 参数: 无；返回: 当前语言下的标题文本。
pub(super) fn changes_heading() -> &'static str {
    t("Changes", "变更")
}

/// 【侧栏国际化】【后台标题】返回后台任务分组标题。
/// 参数: 无；返回: 当前语言下的标题文本。
pub(super) fn background_heading() -> &'static str {
    t("Background tasks", "后台任务")
}

/// 【侧栏国际化】【管理任务】返回打开后台任务面板的入口文本。
/// 参数: 无；返回: 当前语言下的入口标签。
pub(super) fn manage_tasks() -> &'static str {
    t("Manage tasks", "管理任务")
}

/// 【侧栏国际化】【刷新失败】返回后台任务刷新失败状态提示。
/// 参数: 无；返回: 带缩进的状态行文本。
pub(super) fn task_refresh_failed() -> &'static str {
    t("    Task refresh failed", "    任务刷新失败")
}

/// 【侧栏国际化】【主会话】返回切换回主对话的条目名称。
/// 参数: 无；返回: 当前语言下的主会话名称。
pub(super) fn main_conversation() -> &'static str {
    t("Main conversation", "主会话")
}

/// 【侧栏国际化】【当前徽章】返回当前所处会话视图的短状态徽章。
/// 参数: 无；返回: 当前语言下的激活徽章。
pub(super) fn agent_active_badge() -> &'static str {
    t("active", "当前")
}

/// 【侧栏国际化】【子代理状态】将子代理原始状态映射为本地化短词。
/// 参数: `status` 为子代理状态标识；返回: 当前语言下的短状态词。
pub(super) fn subagent_status_label(status: &str) -> &'static str {
    match status {
        "err" => t("err", "错误"),
        "idle" => t("idle", "空闲"),
        _ => t("done", "完成"),
    }
}

/// 【侧栏国际化】【非仓库状态】返回非 Git 仓库的静默标签。
/// 参数: 无；返回: 当前语言下的短标签。
pub(super) fn git_no_repo() -> &'static str {
    t("no repo", "非仓库")
}

/// 【侧栏国际化】【净空状态】返回工作树无改动的静默标签。
/// 参数: 无；返回: 当前语言下的短标签。
pub(super) fn git_clean() -> &'static str {
    t("clean", "无改动")
}

/// 【侧栏国际化】【错误状态】返回 Git 状态读取错误短标签。
/// 参数: 无；返回: 当前语言下的错误标签。
pub(super) fn git_error() -> &'static str {
    t("error", "错误")
}

/// 【侧栏国际化】【不可用说明】返回 Git 状态不可用的详情行。
/// 参数: 无；返回: 带缩进的说明文本。
pub(super) fn git_unavailable() -> &'static str {
    t("    Git status unavailable", "    无法读取 Git 状态")
}

/// 【侧栏国际化】【检查中说明】返回工作树扫描中的详情行。
/// 参数: 无；返回: 带缩进的说明文本。
pub(super) fn git_checking() -> &'static str {
    t("    Checking working tree...", "    正在检查工作树…")
}

/// 【侧栏国际化】【审查标题】返回右侧代码审查面板标题。
/// 参数: 无；返回: 当前语言下的标题文本。
pub(super) fn review_heading() -> &'static str {
    t("Review", "代码审查")
}

/// 【侧栏国际化】【返回概览】返回代码审查顶栏的返回概览按钮文案。
/// 参数: `compact` 为窄栏紧凑模式标志；返回: 当前语言下的返回按钮文本。
pub(super) fn review_back_label(compact: bool) -> &'static str {
    if compact {
        t("← Back", "← 返回")
    } else {
        t("← Overview", "← 概览")
    }
}

/// 【侧栏国际化】【变更文件统计】格式化审查区已变更文件数文案。
/// 参数: `count` 为变更文件数；返回: 加粗后的本地化文件计数字符串。
pub(super) fn files_changed_summary(count: usize) -> String {
    if crate::i18n::is_zh() {
        format!("\x1b[1m{count} 个文件变更\x1b[22m")
    } else {
        format!("\x1b[1m{count} files changed\x1b[22m")
    }
}

/// 【侧栏国际化】【加载变更】返回差异正文加载中的占位提示。
/// 参数: 无；返回: 当前语言下的加载提示。
pub(super) fn loading_changes() -> &'static str {
    t("Loading changes...", "正在加载变更…")
}

/// 【侧栏国际化】【按键聚焦】返回页脚焦点动作短词。
/// 参数: 无；返回: 当前语言下的聚焦短词。
pub(super) fn key_focus() -> &'static str {
    t("focus", "聚焦")
}

/// 【侧栏国际化】【按键打开】返回页脚打开动作短词。
/// 参数: 无；返回: 当前语言下的打开短词。
pub(super) fn key_open() -> &'static str {
    t("open", "打开")
}

/// 【侧栏国际化】【按键选择】返回页脚选择动作短词。
/// 参数: 无；返回: 当前语言下的选择短词。
pub(super) fn key_select() -> &'static str {
    t("select", "选择")
}

/// 【侧栏国际化】【按键全部】返回手风琴退出单区聚焦动作短词。
/// 参数: 无；返回: 当前语言下的全部短词。
pub(super) fn key_all() -> &'static str {
    t("all", "全部")
}

/// 【侧栏国际化】【按键返回】返回页脚返回动作短词。
/// 参数: 无；返回: 当前语言下的返回短词。
pub(super) fn key_return() -> &'static str {
    t("return", "返回")
}

/// 【侧栏国际化】【点击键名】返回未聚焦态页脚鼠标点击词。
/// 参数: 无；返回: 当前语言下的点击词。
pub(super) fn key_click() -> &'static str {
    t("Click", "点击")
}

/// 【侧栏国际化】【折叠提示】返回分组标题悬停时的收起提示。
/// 参数: 无；返回: 当前语言下的收起提示。
pub(super) fn hint_collapse() -> &'static str {
    t("Click to collapse", "点击折叠")
}

/// 【侧栏国际化】【展开提示】返回分组标题悬停时的展开提示。
/// 参数: 无；返回: 当前语言下的展开提示。
pub(super) fn hint_expand() -> &'static str {
    t("Click to expand", "点击展开")
}
