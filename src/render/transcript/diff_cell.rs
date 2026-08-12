use crate::permission::PermissionDecision;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::edit_diff::{edit_diff_stat_status, render_edit_file_diff_for_transcript};
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
    permission: Option<PermissionAuditView>,
    /// 工具是否已结束（成功/失败），避免结果阶段另开空 cell
    completed: Option<bool>,
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
        let rendered = render_edit_file_diff_for_transcript(&arguments).unwrap_or_else(|| {
            // 无法构建 diff 预览时退回状态行：优先展示增删统计，绝不退化成 Write run
            let stats = edit_diff_stat_status(&arguments);
            tool_event_text(
                &tool_event_label_tense(&name, Some(&arguments), ToolVerbTense::Progressive),
                stats.as_deref().unwrap_or(""),
            )
        });
        Self {
            name,
            arguments,
            rendered: rendered.trim_end().to_string(),
            permission: None,
            completed: None,
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

    /// 标记编辑已结束，保留预览 diff。
    ///
    /// 参数:
    /// - `ok`: 是否成功
    ///
    /// 返回:
    /// - 无
    pub(crate) fn finish(&mut self, ok: bool) {
        self.completed = Some(ok);
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
    let stats = edit_diff_stat_status(&cell.arguments);
    // 编辑类摘要始终优先 +N -M 徽标；失败才 err，圆点颜色随结果切换。
    // 禁止成功/进行中落到 Writing run。
    let (badge, health) = match cell.completed {
        Some(false) => (color_status("err"), ToolHealth::Err),
        Some(true) => (stats.unwrap_or_default(), ToolHealth::Ok),
        None => (stats.unwrap_or_default(), ToolHealth::Pending),
    };
    let mut output = tool_status_line(&label, &badge, health);
    // 定稿 DiffCell 必须带上冻结正文；之前只在 Full 挂正文，默认 Summary 下写完后 diff 丢失
    let body = strip_leading_bullet_header(&cell.rendered);
    if !body.trim().is_empty() {
        output.push('\n');
        output.push_str(body.trim_end());
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

/// 去掉冻结预览首行的 `• Added/Edited …` 标题，只保留行级正文。
///
/// 参数:
/// - `rendered`: 冻结的完整预览
///
/// 返回:
/// - 无标题的正文；整段只是状态行时返回空
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
