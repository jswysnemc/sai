use super::command_output_buffer::CommandOutputBuffer;
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
    command_stdout: CommandOutputBuffer,
    command_stderr: CommandOutputBuffer,
    pub(crate) command_expanded: bool,
}

/// 附着在既有工具视图中的权限审计状态。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PermissionAuditView {
    pub(crate) request_id: String,
    pub(crate) selected: PermissionChoice,
    pub(crate) reply_draft: Option<String>,
    pub(crate) decision: Option<PermissionDecision>,
    /// 是否正在并行自动审核（用于 UI 状态行）
    pub(crate) auto_audit: bool,
}

impl PermissionAuditView {
    /// 创建等待用户选择的审计状态。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    ///
    /// 返回:
    /// - 尚未包含决定的审计状态
    #[allow(dead_code)]
    pub(crate) fn pending(request_id: String) -> Self {
        Self::pending_with_auto_audit(request_id, false)
    }

    /// 创建等待决定的审计状态，并标记是否启用自动审核。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `auto_audit`: 是否并行自动审核
    ///
    /// 返回:
    /// - 尚未包含决定的审计状态
    pub(crate) fn pending_with_auto_audit(request_id: String, auto_audit: bool) -> Self {
        Self {
            request_id,
            selected: PermissionChoice::Allow,
            reply_draft: None,
            decision: None,
            auto_audit,
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
            command_stdout: CommandOutputBuffer::default(),
            command_stderr: CommandOutputBuffer::default(),
            command_expanded: false,
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

    /// 追加命令执行期间产生的输出片段。
    ///
    /// 参数:
    /// - `stream`: 输出流类型
    /// - `bytes`: 原始输出字节
    ///
    /// 返回:
    /// - 无
    pub(crate) fn append_command_output(
        &mut self,
        stream: crate::tools::command::CommandOutputStream,
        bytes: &[u8],
        omitted_bytes: usize,
    ) {
        let target = match stream {
            crate::tools::command::CommandOutputStream::Stdout => &mut self.command_stdout,
            crate::tools::command::CommandOutputStream::Stderr => &mut self.command_stderr,
        };
        target.append(bytes, omitted_bytes);
    }

    /// 切换命令输出展开状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 切换后的展开状态
    pub(crate) fn toggle_command_expanded(&mut self) -> bool {
        self.command_expanded = !self.command_expanded;
        self.command_expanded
    }

    /// 判断当前视图是否包含可展开的命令输出。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - run_command 且已有输出时返回 true
    pub(crate) fn has_command_output(&self) -> bool {
        self.name == "run_command"
            && (!self.command_stdout.is_empty()
                || !self.command_stderr.is_empty()
                || self.outcome.is_some())
    }

    /// 返回 stdout 的有界显示文本。
    ///
    /// 返回:
    /// - 包含省略标记的 stdout
    pub(crate) fn command_stdout_text(&self) -> std::borrow::Cow<'_, str> {
        self.command_stdout.display_text()
    }

    /// 返回 stderr 的有界显示文本。
    ///
    /// 返回:
    /// - 包含省略标记的 stderr
    pub(crate) fn command_stderr_text(&self) -> std::borrow::Cow<'_, str> {
        self.command_stderr.display_text()
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
    #[allow(dead_code)]
    pub(crate) fn request_permission(&mut self, request_id: String) {
        self.request_permission_with_auto_audit(request_id, false);
    }

    /// 将权限请求附着到当前工具视图，并标记自动审核。
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
