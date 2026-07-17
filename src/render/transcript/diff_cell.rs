use crate::permission::PermissionDecision;
use crate::render::edit_diff::render_edit_file_diff;
use crate::render::tool_event_line::{tool_event_label, tool_event_text};
use crate::render::tool_view::PermissionAuditView;
use crate::render::PermissionChoice;

/// edit_file 调用时生成的 diff 快照。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiffCell {
    rendered: String,
    permission: Option<PermissionAuditView>,
    /// 工具是否已结束（成功/失败），避免结果阶段另开空 cell
    completed: Option<bool>,
}

impl DiffCell {
    /// 在工具执行前构造 diff 快照。
    ///
    /// 参数:
    /// - `arguments`: edit_file 原始参数
    ///
    /// 返回:
    /// - 不依赖后续文件状态的 diff cell
    pub(crate) fn from_arguments(arguments: String) -> Self {
        let rendered = render_edit_file_diff(&arguments).unwrap_or_else(|| {
            tool_event_text(&tool_event_label("edit_file", Some(&arguments)), "run")
        });
        Self {
            rendered: rendered.trim_end().to_string(),
            permission: None,
            completed: None,
        }
    }

    /// 将权限请求附着到当前 diff 视图。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    ///
    /// 返回:
    /// - 无
    pub(crate) fn request_permission(&mut self, request_id: String) {
        self.permission = Some(PermissionAuditView::pending(request_id));
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

    /// 标记 edit_file 已结束，保留预览 diff 并附状态行。
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
/// 参数:
/// - `cell`: diff 源数据
///
/// 返回:
/// - ANSI 文本块
pub(crate) fn render(cell: &DiffCell) -> String {
    let mut output = cell.rendered.clone();
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
    if let Some(ok) = cell.completed {
        let status = if ok { "ok" } else { "err" };
        output.push_str(&format!(
            "\n{}",
            crate::render::tool_event_line::tool_event_text("Edit", status)
        ));
    }
    output
}
