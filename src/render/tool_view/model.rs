use crate::permission::PermissionDecision;
use crate::render::PermissionChoice;

/// 工具执行的最终结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToolOutcome {
    pub(crate) ok: bool,
    pub(crate) output: String,
}

/// 可在 CLI 与 REPL 间共享的工具生命周期模型。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToolView {
    pub(crate) name: String,
    pub(crate) arguments: String,
    pub(crate) progress: Option<String>,
    pub(crate) outcome: Option<ToolOutcome>,
    pub(crate) permission: Option<PermissionAuditView>,
}

/// 附着在既有工具视图中的权限审计状态。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PermissionAuditView {
    pub(crate) request_id: String,
    pub(crate) selected: PermissionChoice,
    pub(crate) reply_draft: Option<String>,
    pub(crate) decision: Option<PermissionDecision>,
}

impl PermissionAuditView {
    /// 创建等待用户选择的审计状态。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    ///
    /// 返回:
    /// - 尚未包含决定的审计状态
    pub(crate) fn pending(request_id: String) -> Self {
        Self {
            request_id,
            selected: PermissionChoice::Allow,
            reply_draft: None,
            decision: None,
        }
    }

    /// 判断当前状态是否对应指定权限请求。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    ///
    /// 返回:
    /// - 是否对应同一请求
    pub(crate) fn matches(&self, request_id: &str) -> bool {
        self.request_id == request_id
    }
}

impl ToolView {
    /// 创建已经收到完整参数的工具调用视图。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 工具参数
    ///
    /// 返回:
    /// - 运行中的工具视图
    pub(crate) fn running(name: String, arguments: String) -> Self {
        Self {
            name,
            arguments,
            progress: None,
            outcome: None,
            permission: None,
        }
    }

    /// 创建仅用于参数流预览的工具视图。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments_preview`: 尚未完成的参数文本
    ///
    /// 返回:
    /// - 参数接收中的工具视图
    pub(crate) fn preparing(name: String, arguments_preview: String) -> Self {
        Self::running(name, arguments_preview)
    }

    /// 更新工具进度信息。
    ///
    /// 参数:
    /// - `message`: 最新进度文本
    pub(crate) fn set_progress(&mut self, message: String) {
        self.progress = Some(message);
    }

    /// 完成工具调用。
    ///
    /// 参数:
    /// - `ok`: 工具是否成功
    /// - `output`: 工具输出
    pub(crate) fn finish(&mut self, ok: bool, output: String) {
        self.outcome = Some(ToolOutcome { ok, output });
    }

    /// 将权限请求附着到当前工具视图。
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
    /// - 是否更新了当前工具视图
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

    /// 更新权限请求中的高亮选项。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `selected`: 当前高亮选项
    ///
    /// 返回:
    /// - 是否更新了当前工具视图
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
    /// - 是否更新了当前工具视图
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

    /// 判断视图是否属于指定工具且仍在运行。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 是否可以继续更新该视图
    pub(crate) fn is_active_for(&self, name: &str) -> bool {
        self.name == name && self.outcome.is_none()
    }
}
