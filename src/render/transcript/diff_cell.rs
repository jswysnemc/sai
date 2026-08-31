use crate::permission::PermissionDecision;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::edit_diff::{
    edit_diff_stat_status, format_diff_stat_status, preview_from_arguments,
    render_patch_preview_for_transcript,
};
use crate::render::status_style::{color_status, ToolHealth};
use crate::render::style::TOOL_BULLET;
use crate::render::tool_event_line::{
    tool_event_label_tense, tool_event_text, tool_status_line, ToolVerbTense,
};
use crate::render::tool_view::PermissionAuditView;
use crate::render::{PermissionChoice, ToolCallDisplayMode};

/// 编辑类工具在执行前冻结的 diff 快照。
///
/// 摘要行用 `Write/Replace path +N -M`；Summary/Full 都挂冻结的行级正文
/// （参数流阶段的跳动统计仍走 live ToolView，不会提前倾倒 diff）。
/// 正文在写盘前生成，避免 str_replace 执行后无法从磁盘重建预览。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiffCell {
    name: String,
    arguments: String,
    /// 冻结的完整预览（可含旧式 Added 标题；渲染时会剥掉）
    rendered: String,
    /// 与冻结预览同源的 `+N -M` 统计
    ///
    /// 写盘后原文件已变，重新预览会得到空 diff；统计必须和正文一起冻结，
    /// 否则徽标会比实际画出的 +/- 行少。
    stats: Option<String>,
    permission: Option<PermissionAuditView>,
    /// 工具是否已结束（成功/失败），避免结果阶段另开空 cell
    completed: Option<bool>,
    /// 超长 diff 是否已展开；Ctrl+O 切换
    expanded: bool,
}

impl DiffCell {
    /// 在工具执行前构造 diff 快照。
    ///
    /// 参数:
    /// - `name`: 工具名称（`write_file` / `str_replace` / `edit_file`）
    /// - `arguments`: 原始工具参数
    ///
    /// 返回:
    /// - 不依赖后续文件状态的 diff cell
    pub(crate) fn from_call(name: String, arguments: String) -> Self {
        // 正文与统计共用同一次预览：两处都按写盘前的文件状态冻结
        let preview = preview_from_arguments(&arguments).ok();
        let stats = preview
            .as_ref()
            .map(|preview| {
                let (added, removed) = preview.line_counts();
                format_diff_stat_status(added, removed)
            })
            // 预览失败时退回宽松扫描，让状态行不退化成 Write run
            .or_else(|| edit_diff_stat_status(&arguments));
        let rendered = preview
            .as_ref()
            .map(render_patch_preview_for_transcript)
            .unwrap_or_else(|| {
                // 无法构建 diff 预览时退回状态行：优先展示增删统计，绝不退化成 Write run
                tool_event_text(
                    &tool_event_label_tense(&name, Some(&arguments), ToolVerbTense::Progressive),
                    stats.as_deref().unwrap_or(""),
                )
            });
        Self {
            name,
            arguments,
            rendered: rendered.trim_end().to_string(),
            stats,
            permission: None,
            completed: None,
            expanded: false,
        }
    }

    /// 兼容旧构造：默认按 `edit_file` 处理。
    ///
    /// 参数:
    /// - `arguments`: edit_file 原始参数
    ///
    /// 返回:
    /// - diff cell
    #[allow(dead_code)]
    pub(crate) fn from_arguments(arguments: String) -> Self {
        Self::from_call("edit_file".to_string(), arguments)
    }

    /// 将权限请求附着到当前 diff 视图。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    ///
    /// 返回:
    /// - 无
    #[allow(dead_code)]
    pub(crate) fn request_permission(&mut self, request_id: String) {
        self.request_permission_with_auto_audit(request_id, false);
    }

    /// 将权限请求附着到 diff 视图，并标记自动审核。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `auto_audit`: 是否并行自动审核
    ///
    /// 返回:
    /// - 无
    pub(crate) fn request_permission_with_auto_audit(
        &mut self,
        request_id: String,
        auto_audit: bool,
    ) {
        self.permission = Some(PermissionAuditView::pending_with_auto_audit(
            request_id, auto_audit,
        ));
    }

    /// 写入权限请求的最终决定。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `decision`: 用户决定
    ///
    /// 返回:
    /// - 是否更新了当前 diff 视图
    pub(crate) fn resolve_permission(
        &mut self,
        request_id: &str,
        decision: PermissionDecision,
    ) -> bool {
        let Some(permission) = self.permission.as_mut() else {
            return false;
        };
        if !permission.matches(request_id) {
            return false;
        }
        permission.decision = Some(decision);
        permission.reply_draft = None;
        true
    }

    /// 更新权限请求的高亮选项。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `selected`: 当前高亮选项
    ///
    /// 返回:
    /// - 是否更新了当前 diff 视图
    pub(crate) fn set_permission_choice(
        &mut self,
        request_id: &str,
        selected: PermissionChoice,
    ) -> bool {
        let Some(permission) = self.permission.as_mut() else {
            return false;
        };
        if !permission.matches(request_id) {
            return false;
        }
        permission.selected = selected;
        true
    }

    /// 更新权限拒绝回复草稿。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `draft`: 回复草稿
    ///
    /// 返回:
    /// - 是否更新了当前 diff 视图
    pub(crate) fn set_permission_reply(&mut self, request_id: &str, draft: Option<String>) -> bool {
        let Some(permission) = self.permission.as_mut() else {
            return false;
        };
        if !permission.matches(request_id) {
            return false;
        }
        permission.reply_draft = draft;
        true
    }

    /// 切换超长 diff 的展开状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[cfg(test)]
    pub(crate) fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    /// 标记编辑已结束，保留预览 diff。
    pub(crate) fn finish(&mut self, ok: bool) {
        self.completed = Some(ok);
    }

    /// 编辑是否仍在执行。
    pub(crate) fn is_pending(&self) -> bool {
        self.completed.is_none()
    }

    /// 返回 Ctrl+O 分页标题。
    ///
    /// 返回:
    /// - 工具名
    pub(crate) fn pager_title(&self) -> String {
        self.name.clone()
    }

    /// 返回已渲染的完整 diff 正文（去掉旧式标题行）。
    ///
    /// 返回:
    /// - 可直接写入 pager 的 ANSI 文本
    pub(crate) fn pager_body(&self) -> String {
        strip_leading_bullet_header(&self.rendered)
    }
}

/// 渲染已固化的 diff 快照。
///
/// 摘要行：`Write/Replace path +N -M`。
/// Summary/Full：再挂冻结行级正文（剥掉旧式 `• Added` 标题）；Hidden 不渲染。
///
/// 参数:
/// - `cell`: diff 源数据
/// - `mode`: 工具展示模式
///
/// 返回:
/// - ANSI 文本块
pub(crate) fn render(cell: &DiffCell, mode: ToolCallDisplayMode) -> String {
    if mode == ToolCallDisplayMode::Hidden {
        return String::new();
    }
    let tense = ToolVerbTense::from_done(cell.completed.is_some());
    let label = tool_event_label_tense(&cell.name, Some(&cell.arguments), tense);
    // 编辑类摘要始终优先 +N -M 徽标；失败才 err，圆点颜色随结果切换。
    // 禁止成功/进行中落到 Writing run。
    let (badge, health) = match cell.completed {
        Some(false) => (color_status("err"), ToolHealth::Err),
        Some(true) => (cell.stats.clone().unwrap_or_default(), ToolHealth::Ok),
        None => (cell.stats.clone().unwrap_or_default(), ToolHealth::Pending),
    };
    let mut output = tool_status_line(&label, &badge, health);
    // 定稿 DiffCell 必须带上冻结正文；之前只在 Full 挂正文，默认 Summary 下写完后 diff 丢失
    let body = strip_leading_bullet_header(&cell.rendered);
    if !body.trim().is_empty() {
        output.push('\n');
        output.push_str(fold_diff_body(body.trim_end(), cell.expanded).trim_end());
    }

    // 权限控件与其他工具卡一致顶正文列，不再随 diff 正文内收
    if let Some(permission) = &cell.permission {
        match &permission.decision {
            Some(decision) => {
                if !output.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str(&crate::render::render_permission_decision(decision));
            }
            None => {
                if !output.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str(&crate::render::render_permission_controls(
                    permission.selected,
                    permission.reply_draft.as_deref(),
                ));
            }
        }
    }
    output
}

/// 折叠前保留的 diff 头部行数。
///
/// 比命令输出的头尾更宽：一处改动连同上下文就占 7 行，
/// 按命令输出的 2/4 折，单点修改也会被折成省略号。
const DIFF_FOLD_HEAD_LINES: usize = 8;

/// 折叠后保留的 diff 尾部行数。
const DIFF_FOLD_TAIL_LINES: usize = 8;

/// 超长 diff 正文按首尾折叠，中间以省略行代替。
///
/// 按 diff 自身的行折而不是终端显示行：diff 每行都带行号与增删标记，
/// 折行产生的续行不是独立信息，按显示行折会把一处改动拦腰截断。
///
/// 参数:
/// - `body`: 无标题的 diff 正文
/// - `expanded`: 该 cell 是否已展开
///
/// 返回:
/// - 折叠后的正文；未超长或已展开时原样返回
fn fold_diff_body(body: &str, expanded: bool) -> String {
    if expanded || crate::render::render_expand::expand_override() {
        return body.to_string();
    }
    let lines: Vec<&str> = body.lines().collect();
    let keep = DIFF_FOLD_HEAD_LINES + DIFF_FOLD_TAIL_LINES;
    if lines.len() <= keep {
        return body.to_string();
    }
    let omitted = lines.len() - keep;
    // 用「省略 N 行」而不是「+N 行」：后者会被读成新增行数，与徽标的 +N 撞车
    let hint = format!(
        "\x1b[2m\x1b[36m  └ … {} {omitted} {} (Ctrl+O {})\x1b[0m",
        crate::i18n::text("omitted", "省略"),
        crate::i18n::text("lines", "行"),
        crate::i18n::text("to expand", "展开")
    );
    let mut output: Vec<String> = lines[..DIFF_FOLD_HEAD_LINES]
        .iter()
        .map(|line| (*line).to_string())
        .collect();
    output.push(hint);
    output.extend(
        lines[lines.len() - DIFF_FOLD_TAIL_LINES..]
            .iter()
            .map(|line| (*line).to_string()),
    );
    output.join("\n")
}

/// 去掉冻结预览首行的 `• Added/Edited …` 标题，只保留行级正文。
fn strip_leading_bullet_header(rendered: &str) -> String {
    let mut lines = rendered.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let plain = strip_ansi_for_test(first);
    let trimmed = plain.trim_start();
    if trimmed.starts_with(TOOL_BULLET) {
        return lines.collect::<Vec<_>>().join("\n");
    }
    rendered.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ToolCallDisplayMode;

    /// 构造一次整文件重写的 diff cell。
    ///
    /// 参数:
    /// - `dir`: 临时目录
    /// - `old_lines`: 原文件行数
    ///
    /// 返回:
    /// - diff cell
    fn rewrite_cell(dir: &std::path::Path, old_lines: usize) -> DiffCell {
        let path = dir.join("sample.txt");
        let old: String = (1..=old_lines).map(|n| format!("line{n}\n")).collect();
        let new: String = (1..=old_lines).map(|n| format!("CHANGED{n}\n")).collect();
        std::fs::write(&path, old).unwrap();
        let args = serde_json::json!({
            "command": "write_file",
            "path": path.to_string_lossy(),
            "content": new
        })
        .to_string();
        DiffCell::from_call("write_file".to_string(), args)
    }

    /// 【TUI】【diff 折叠】超长 diff 折叠为首尾并给出 Ctrl+O 提示。
    ///
    /// 一次整文件重写会刷掉整屏，长度本身就是需要被收起的信号。
    #[test]
    fn long_diff_folds_into_head_and_tail() {
        let dir = tempfile::tempdir().unwrap();
        let mut cell = rewrite_cell(dir.path(), 60);

        let folded = strip_ansi_for_test(&render(&cell, ToolCallDisplayMode::Summary));
        assert!(folded.contains("Ctrl+O"), "折叠后应提示展开方式:\n{folded}");
        let folded_rows = folded.lines().count();

        cell.toggle_expanded();
        let expanded = strip_ansi_for_test(&render(&cell, ToolCallDisplayMode::Summary));
        assert!(!expanded.contains("Ctrl+O"), "展开后不应再有提示");
        assert!(
            expanded.lines().count() > folded_rows,
            "展开后行数应多于折叠"
        );
    }

    /// 【TUI】【diff 折叠】短 diff 保持原样，不插入省略行。
    ///
    /// 单点修改连同上下文只有几行，折起来反而要多按一次键才能读全。
    #[test]
    fn short_diff_is_not_folded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.txt");
        std::fs::write(&path, "a\nb\nc\n").unwrap();
        let args = serde_json::json!({
            "command": "str_replace",
            "path": path.to_string_lossy(),
            "old_string": "b",
            "new_string": "B"
        })
        .to_string();

        let cell = DiffCell::from_call("str_replace".to_string(), args);

        assert!(
            !strip_ansi_for_test(&render(&cell, ToolCallDisplayMode::Summary)).contains("Ctrl+O")
        );
    }

    /// 【TUI】【diff 折叠】回看上下文里按展开渲染。
    ///
    /// 备用屏浏览的用途就是读全文，折叠块在那里要全部摊开。
    #[test]
    fn review_context_expands_every_diff() {
        let dir = tempfile::tempdir().unwrap();
        let cell = rewrite_cell(dir.path(), 60);

        let rendered = crate::render::render_expand::with_expanded_render(|| {
            strip_ansi_for_test(&render(&cell, ToolCallDisplayMode::Summary))
        });

        assert!(!rendered.contains("Ctrl+O"));
    }

    /// 统计正文里实际画出的 `+` / `-` 行条数。
    ///
    /// 正文行形如 `  3 -  two` / `  3 +  TWO`；状态行的 `+1 -1` 徽标不带
    /// 行号列，不会被计进来。
    ///
    /// 参数:
    /// - `rendered`: 已渲染的 diff 视图
    ///
    /// 返回:
    /// - `(新增行数, 删除行数)`
    fn rendered_marker_rows(rendered: &str) -> (usize, usize) {
        let plain = strip_ansi_for_test(rendered);
        let mut added = 0usize;
        let mut removed = 0usize;
        for line in plain.lines() {
            let trimmed = line.trim_start();
            let after_number = trimmed.trim_start_matches(|ch: char| ch.is_ascii_digit());
            if after_number.len() == trimmed.len() {
                continue;
            }
            let body = after_number.strip_prefix(' ').unwrap_or(after_number);
            if body.starts_with("+ ") {
                added += 1;
            } else if body.starts_with("- ") {
                removed += 1;
            }
        }
        (added, removed)
    }

    /// 【TUI】【diff 统计】徽标的增删行数必须等于正文画出的 +/- 行。
    ///
    /// 此前 write_file 覆盖已有文件只按 content 计新增，删除侧恒为 0，
    /// 于是出现「+3 -0」的徽标配 3 红 3 绿六行正文。
    #[test]
    fn stat_badge_matches_the_rendered_marker_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        let args = serde_json::json!({
            "command": "write_file",
            "path": path.to_string_lossy(),
            "content": "ONE\nTWO\nTHREE\n"
        })
        .to_string();

        let mut cell = DiffCell::from_call("write_file".to_string(), args);
        cell.finish(true);
        let rendered = render(&cell, ToolCallDisplayMode::Full);
        let (added, removed) = rendered_marker_rows(&rendered);
        assert_eq!((added, removed), (3, 3), "正文行数不对:\n{rendered}");

        let plain = strip_ansi_for_test(&rendered);
        let status = plain.lines().next().unwrap_or_default();
        assert!(status.contains("+3"), "徽标应报 +3: {status}");
        assert!(status.contains("-3"), "徽标应报 -3: {status}");
    }

    /// 【TUI】【diff 统计】写盘后文件已变，冻结的统计不能跟着塌成 +0 -0。
    ///
    /// 统计若改成渲染时重算预览，读到的就是改完的新内容，diff 为空。
    #[test]
    fn frozen_stats_survive_the_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        let new = "ONE\nTWO\nTHREE\n";
        let args = serde_json::json!({
            "command": "write_file",
            "path": path.to_string_lossy(),
            "content": new
        })
        .to_string();

        let mut cell = DiffCell::from_call("write_file".to_string(), args);
        // 模拟工具已落盘：此后才渲染
        std::fs::write(&path, new).unwrap();
        cell.finish(true);

        let plain = strip_ansi_for_test(&render(&cell, ToolCallDisplayMode::Full));
        let status = plain.lines().next().unwrap_or_default();
        assert!(status.contains("+3"), "写盘后徽标不应塌掉: {status}");
        assert!(status.contains("-3"), "写盘后徽标不应塌掉: {status}");
    }

    /// 【TUI】【diff 统计】str_replace 只报真正改动的行。
    ///
    /// 按 old/new_string 的行数计会把「三行里改一行」报成 +3 -3。
    #[test]
    fn str_replace_badge_counts_only_changed_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "a\nb\nc\n").unwrap();
        let args = serde_json::json!({
            "command": "str_replace",
            "path": path.to_string_lossy(),
            "old_string": "a\nb\nc\n",
            "new_string": "a\nB\nc\n"
        })
        .to_string();

        let mut cell = DiffCell::from_call("str_replace".to_string(), args);
        cell.finish(true);
        let rendered = render(&cell, ToolCallDisplayMode::Full);
        assert_eq!(
            rendered_marker_rows(&rendered),
            (1, 1),
            "正文行数不对:\n{rendered}"
        );
        let status = strip_ansi_for_test(&rendered);
        let status = status.lines().next().unwrap_or_default();
        assert!(status.contains("+1"), "徽标应报 +1: {status}");
        assert!(status.contains("-1"), "徽标应报 -1: {status}");
    }

    /// 【TUI】【diff 统计】追加模式只计新增，不把原内容画成删除。
    #[test]
    fn append_mode_counts_only_the_appended_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.txt");
        std::fs::write(&path, "first\n").unwrap();
        let args = serde_json::json!({
            "command": "write_file",
            "path": path.to_string_lossy(),
            "content": "second\n",
            "mode": "append"
        })
        .to_string();

        let mut cell = DiffCell::from_call("write_file".to_string(), args);
        cell.finish(true);
        let rendered = render(&cell, ToolCallDisplayMode::Full);
        let (added, removed) = rendered_marker_rows(&rendered);
        assert_eq!(removed, 0, "追加不应删行:\n{rendered}");
        assert_eq!(added, 1, "追加应只加一行:\n{rendered}");
        let status = strip_ansi_for_test(&rendered);
        let status = status.lines().next().unwrap_or_default();
        assert!(status.contains("+1"), "徽标应报 +1: {status}");
        assert!(status.contains("-0"), "徽标应报 -0: {status}");
    }
}
