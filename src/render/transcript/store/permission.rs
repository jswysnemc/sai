use super::*;
use crate::render::tool_view::ToolView;
use crate::render::transcript::tool_cell::ToolCell;

impl TranscriptStore {
    /// 记录等待用户选择的权限事件。
    ///
    /// 参数:
    /// - `request`: 权限请求
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_permission_request(
        &mut self,
        request: crate::permission::PermissionRequest,
    ) {
        self.finalize_live_tail();
        // 优先挂到当前活动工具；若活动索引已漂移，回退扫描最近匹配工具，避免审计 UI 丢失
        if let Some(index) = self.active_tool_index {
            if self.attach_permission_at(index, &request) {
                return;
            }
        }
        for index in (0..self.cells.len()).rev() {
            if self.attach_permission_at(index, &request) {
                self.active_tool_index = Some(index);
                return;
            }
        }
        // 1. 【权限】【独立卡片】缺少对应调用时创建可见审批载体，禁止静默丢弃请求
        let mut view = ToolView::running(request.tool.clone(), request.arguments.clone());
        view.request_permission_with_auto_audit(request.id.clone(), request.auto_audit);
        view.set_permission_sandbox(request.sandbox.clone());
        self.push_cell(HistoryCell::Tool(ToolCell::Invocation(Box::new(view))));
    }

    /// 更新指定权限事件的最终决定。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `decision`: 用户决定
    ///
    /// 返回:
    /// - 是否找到并更新了权限事件
    pub(crate) fn resolve_permission(
        &mut self,
        request_id: &str,
        decision: crate::permission::PermissionDecision,
    ) -> bool {
        self.update_permission_cells(|cell| match cell {
            HistoryCell::Tool(ToolCell::Invocation(view)) => {
                view.resolve_permission(request_id, decision.clone())
            }
            HistoryCell::Diff(cell) => cell.resolve_permission(request_id, decision.clone()),
            _ => false,
        })
    }

    /// 更新指定权限事件的拒绝回复草稿。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `draft`: 回复草稿；空值表示返回权限选择
    ///
    /// 返回:
    /// - 是否找到并更新了权限事件
    pub(crate) fn set_permission_reply_draft(
        &mut self,
        request_id: &str,
        draft: Option<String>,
    ) -> bool {
        self.update_permission_cells(|cell| match cell {
            HistoryCell::Tool(ToolCell::Invocation(view)) => {
                view.set_permission_reply(request_id, draft.clone())
            }
            HistoryCell::Diff(cell) => cell.set_permission_reply(request_id, draft.clone()),
            _ => false,
        })
    }

    /// 更新指定权限事件的当前高亮选项。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `selected`: 高亮选项
    ///
    /// 返回:
    /// - 是否找到并更新了权限事件
    pub(crate) fn set_permission_choice(
        &mut self,
        request_id: &str,
        selected: crate::render::PermissionChoice,
    ) -> bool {
        self.update_permission_cells(|cell| match cell {
            HistoryCell::Tool(ToolCell::Invocation(view)) => {
                view.set_permission_choice(request_id, selected)
            }
            HistoryCell::Diff(cell) => cell.set_permission_choice(request_id, selected),
            _ => false,
        })
    }

    /// 从最新 cell 向前查找并更新权限事件，命中时标脏。
    ///
    /// 参数:
    /// - `update`: 权限更新函数，命中时返回 true
    ///
    /// 返回:
    /// - 是否找到并更新了权限事件
    fn update_permission_cells<F>(&mut self, mut update: F) -> bool
    where
        F: FnMut(&mut HistoryCell) -> bool,
    {
        for index in (0..self.cells.len()).rev() {
            if update(&mut self.cells[index]) {
                self.mark_dirty(index);
                return true;
            }
        }
        false
    }

    /// 将权限审计挂到指定 transcript 单元。
    ///
    /// 参数:
    /// - `index`: cell 索引
    /// - `request`: 权限请求
    ///
    /// 返回:
    /// - 是否成功附着
    fn attach_permission_at(
        &mut self,
        index: usize,
        request: &crate::permission::PermissionRequest,
    ) -> bool {
        let attached = match self.cells.get_mut(index) {
            Some(HistoryCell::Tool(ToolCell::Invocation(view)))
                if view.outcome.is_none()
                    && (view.name == request.tool || view.name == "codemode") =>
            {
                view.request_permission_with_auto_audit(request.id.clone(), request.auto_audit);
                view.set_permission_sandbox(request.sandbox.clone());
                if let Some(audit) = view.permission.as_mut() {
                    audit.tool = Some(request.tool.clone());
                    audit.arguments = Some(request.arguments.clone());
                }
                true
            }
            Some(HistoryCell::Diff(cell))
                if cell.is_pending()
                    && crate::render::stream_text::is_file_edit_tool(&request.tool) =>
            {
                cell.request_permission_with_auto_audit(request.id.clone(), request.auto_audit);
                true
            }
            _ => false,
        };
        if attached {
            self.mark_dirty(index);
        }
        attached
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

    /// 【权限】【嵌套展示回归】无参数；验证脚本卡片展示真实命令与审批选项，拒绝后按请求标识更新。
    #[test]
    fn nested_permission_displays_real_operation_on_outer_card() {
        let mut store = TranscriptStore::new(100);
        store.push_tool_call(
            "codemode".into(),
            r#"{"code":"await tools.run_command({})"}"#.into(),
        );
        store.push_permission_request(crate::permission::PermissionRequest {
            id: "nested-approval".into(),
            session_id: "session".into(),
            tool: "run_command".into(),
            arguments: r#"{"command":"printf approved > result.txt"}"#.into(),
            auto_audit: false,
            sandbox: None,
        });
        let options = TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Summary,
            tool_call_mode: ToolCallDisplayMode::Full,
        };
        let text = store
            .display_tail(100, &options)
            .iter()
            .map(|line| line.as_str())
            .collect::<String>();
        assert!(text.contains("Allow once"), "{text}");
        assert!(text.contains("printf approved"), "{text}");
        assert!(text.contains("run_command"), "{text}");
        assert_eq!(store.cells.len(), 1);
        assert!(store.resolve_permission(
            "nested-approval",
            crate::permission::PermissionDecision::Deny { reply: None }
        ));
    }

    /// 【权限】【缺失卡片回归】无参数；验证未知调用也保留可见审批载体。
    #[test]
    fn missing_tool_card_never_discards_permission() {
        let mut store = TranscriptStore::new(100);
        store.push_permission_request(crate::permission::PermissionRequest {
            id: "standalone".into(),
            session_id: "session".into(),
            tool: "plugin_action".into(),
            arguments: "{}".into(),
            auto_audit: false,
            sandbox: None,
        });
        assert!(store.set_permission_choice("standalone", crate::render::PermissionChoice::Deny));
        assert_eq!(store.cells.len(), 1);
    }
}
