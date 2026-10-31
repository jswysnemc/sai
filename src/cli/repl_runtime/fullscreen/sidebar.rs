//! 右侧概览的独立状态与鼠标分流。

use super::git_preview::GitPreview;
use super::input::Effect;
use super::layout::FullscreenLayout;
use super::sidebar_action::{SidebarAction, SidebarSections};
use super::sidebar_git::GitWatcher;
use super::sidebar_regions::{regions, SidebarScrolls};
use super::sidebar_view::{SidebarDocument, SidebarRow};
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

#[derive(Default)]
pub(super) struct Sidebar {
    pub(super) git: GitWatcher,
    pub(super) document: SidebarDocument,
    pub(super) scrolls: SidebarScrolls,
    pub(super) hovered: Option<usize>,
    pub(super) sections: SidebarSections,
    pub(super) focused: bool,
    pub(super) focused_row: Option<usize>,
    pub(super) preview: GitPreview,
}

impl Sidebar {
    /// 【终端侧栏】【文档同步】更新概览，内容收缩后限制独立滚动位置。
    /// 参数: document 为概览行，height 为可见高度；返回: 无。
    pub(super) fn update(&mut self, document: SidebarDocument, height: usize) {
        // 1. 【终端侧栏】【悬停同步】异步刷新移动条目后不保留旧动作的高亮
        self.hovered = self.hovered.filter(|index| {
            self.document.rows.get(*index).and_then(|row| row.action)
                == document.rows.get(*index).and_then(|row| row.action)
        });
        self.document = document;
        self.focused_row = self.focused_row.filter(|index| {
            self.document
                .rows
                .get(*index)
                .is_some_and(|row| row.action.is_some())
        });
        if self.focused && self.focused_row.is_none() {
            self.focused_row = self
                .document
                .rows
                .iter()
                .position(|row| row.action.is_some());
        }
        for region in regions(&self.document, height) {
            if self.scrolls.scroll(&region, 0) {
                self.hovered = None;
            }
        }
        self.hovered = self
            .hovered
            .filter(|index| *index < self.document.rows.len());
    }

    /// 【终端侧栏】【条目命中】可操作条目支持整行点击，分隔线、页脚和只读详情不响应。
    /// 参数: layout 为屏幕分区，column/row 为坐标；返回: 可点击条目下标。
    pub(super) fn target_at(
        &self,
        layout: &FullscreenLayout,
        column: u16,
        row: u16,
    ) -> Option<usize> {
        let sidebar = layout.sidebar?;
        if !layout.in_sidebar(column, row)
            || layout.sidebar_close_at(column, row)
            || row.saturating_add(1) >= layout.composer_top
            || column <= sidebar.left
            || column >= layout.cols.saturating_sub(1)
        {
            return None;
        }
        let index = regions(&self.document, usize::from(layout.composer_top))
            .iter()
            .find_map(|region| region.index_at(usize::from(row), &self.scrolls))?;
        let entry = self.document.rows.get(index)?;
        entry.action.map(|_| index)
    }
    /// 【终端侧栏】【可见行定位】通过分组几何查找文档行，不依赖全局滚动偏移。
    /// 参数: layout 为布局，index 为文档行；返回: 屏幕行。
    #[cfg(test)]
    pub(super) fn screen_row(&self, layout: &FullscreenLayout, index: usize) -> Option<u16> {
        regions(&self.document, usize::from(layout.composer_top))
            .iter()
            .find_map(|region| region.row_for(index, &self.scrolls))
            .map(|row| row as u16)
    }
}

impl ReplRuntime {
    /// 【终端侧栏】【布局判断】使用真实终端尺寸决定底部概览是否迁移到侧栏。
    /// 参数: cols 为终端宽度；返回: 是否启用侧栏。
    pub(in crate::cli::repl_runtime) fn sidebar_visible(&self, cols: usize) -> bool {
        self.fullscreen.as_ref().is_some_and(|session| {
            self.sidebar_control
                .geometry(
                    cols.min(u16::MAX as usize) as u16,
                    super::super::viewport::TerminalSize::current().rows,
                    session.sidebar.preview.path.is_some(),
                )
                .is_some()
        })
    }

    /// 【终端侧栏】【Git 工作目录】查看隔离子任务时使用其 worktree，否则使用当前工作目录。
    /// 参数: 无；返回: 需要查询的工作目录。
    pub(super) fn sidebar_directory(&self) -> std::path::PathBuf {
        self.transcript
            .viewing_subagent_id()
            .and_then(|id| crate::tools::subagent_state::subagent_snapshot(id).ok())
            .and_then(|snapshot| snapshot.worktree_root)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| crate::runtime_cwd::current_dir().unwrap_or_default())
    }

    /// 【终端侧栏】【Git 采样】取回异步结果，必要时启动下一次查询。
    /// 参数: 无；返回: Git 概览是否变化。
    pub(in crate::cli::repl_runtime) fn tick_sidebar_git(&mut self) -> bool {
        let directory = self.sidebar_directory();
        let preview_changed = self
            .fullscreen
            .as_mut()
            .is_some_and(|session| session.sidebar.preview.tick(&directory));
        let size = super::super::viewport::TerminalSize::current();
        if !self.sidebar_visible(usize::from(size.cols)) {
            return preview_changed;
        }
        self.fullscreen
            .as_mut()
            .is_some_and(|session| session.sidebar.git.tick(directory))
            || preview_changed
    }

    /// 【终端侧栏】【定时唤醒】让空闲输入和流式输入均能刷新 Git 概览。
    /// 参数: 无；返回: 侧栏下一次刷新等待，隐藏时为空。
    pub(in crate::cli::repl_runtime) fn sidebar_wait(&self) -> Option<std::time::Duration> {
        let preview = self
            .fullscreen
            .as_ref()
            .and_then(|session| session.sidebar.preview.wait());
        let git = self
            .sidebar_visible(usize::from(
                super::super::viewport::TerminalSize::current().cols,
            ))
            .then(|| self.fullscreen.as_ref().unwrap().sidebar.git.wait());
        match (preview, git) {
            (Some(preview), Some(git)) => Some(preview.min(git)),
            (preview, git) => preview.or(git),
        }
    }

    /// 【终端侧栏】【鼠标隔离】侧栏滚轮和点击不传给会话滚动、选择或输入框。
    /// 参数: mouse 为鼠标事件；返回: 处理效果，未命中侧栏时为空。
    pub(super) fn sidebar_mouse(&mut self, mouse: MouseEvent) -> Option<Effect> {
        let session = self.fullscreen.as_mut()?;
        let layout = session.state.layout?;
        if layout.review {
            return None;
        }
        if !layout.in_sidebar(mouse.column, mouse.row) {
            if session.sidebar.hovered.take().is_some() && mouse.kind == MouseEventKind::Moved {
                return Some(Effect::Repaint);
            }
            return None;
        }
        let cleared = session.state.pointer_row.take().is_some()
            | session.state.pointer_col.take().is_some()
            | session.state.hover.take().is_some()
            | std::mem::take(&mut session.state.header_hover);
        let sidebar = &mut session.sidebar;
        let index = sidebar.target_at(&layout, mouse.column, mouse.row);
        let mut action = None;
        let changed = match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let region = regions(&sidebar.document, usize::from(layout.composer_top))
                    .into_iter()
                    .find(|region| {
                        usize::from(mouse.row) >= region.top
                            && usize::from(mouse.row) <= region.top + region.visible
                    });
                let delta = if mouse.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                let changed = region.is_some_and(|region| sidebar.scrolls.scroll(&region, delta));
                let hover_cleared = sidebar.hovered.take().is_some();
                changed || hover_cleared
            }
            MouseEventKind::Moved => {
                let changed = sidebar.hovered != index;
                sidebar.hovered = index;
                changed
            }
            MouseEventKind::Down(MouseButton::Left) => {
                action = index.and_then(|index| sidebar.document.rows[index].action);
                sidebar.focused_row = index;
                let changed = sidebar.hovered.take().is_some();
                session.state.press = None;
                session.state.selection = None;
                session.state.dragging = false;
                changed
            }
            _ => false,
        };
        if let Some(action) = action {
            self.apply_sidebar_action(action);
            return Some(Effect::Refresh);
        }
        Some(if changed || cleared {
            Effect::Repaint
        } else {
            Effect::Nothing
        })
    }

    /// 【终端侧栏】【执行动作】分组、会话和文件操作使用独立动作，不解析界面文字。
    /// 参数: action 为命中的动作；返回: 无。
    pub(super) fn apply_sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::Toggle(section) => self
                .fullscreen
                .as_mut()
                .unwrap()
                .sidebar
                .sections
                .toggle(section),
            SidebarAction::Panel(action) => self.apply_panel_action(action),
            SidebarAction::GitFile(index) => {
                let path = self
                    .fullscreen
                    .as_ref()
                    .and_then(|session| session.sidebar.git.summary.files.get(index))
                    .map(|(_, path)| path.clone());
                if let Some(path) = path {
                    let directory = self.sidebar_directory();
                    let session = self.fullscreen.as_mut().unwrap();
                    session.state.message_picker.close();
                    session.sidebar.preview.open(directory, path);
                }
            }
        }
    }

    /// 【终端侧栏】【键盘焦点】F6 切换侧栏焦点，方向键选择条目，确认执行并保持输入草稿。
    /// 参数: event 为终端事件；返回: 已处理的绘制效果，不属于侧栏时为空。
    pub(super) fn sidebar_key(&mut self, event: &Event) -> Option<Effect> {
        if matches!(event, Event::Paste(_))
            && self.fullscreen.as_ref().is_some_and(|session| {
                session.sidebar.focused
                    && session
                        .state
                        .layout
                        .is_some_and(|layout| layout.sidebar.is_some())
            })
        {
            return Some(Effect::Nothing);
        }
        let Event::Key(key) = event else {
            return None;
        };
        if key.kind == KeyEventKind::Release {
            return None;
        }
        let session = self.fullscreen.as_mut()?;
        let layout = session.state.layout?;
        let sidebar = &mut session.sidebar;
        if layout.review {
            return None;
        }
        if key.code == KeyCode::F(6) && layout.sidebar.is_some() {
            sidebar.focused = !sidebar.focused;
            sidebar.hovered = None;
            sidebar.focused_row = sidebar.focused_row.or_else(|| {
                sidebar
                    .document
                    .rows
                    .iter()
                    .position(|row| row.action.is_some())
            });
            return Some(Effect::Repaint);
        }
        if !sidebar.focused || layout.sidebar.is_none() {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'o'))
        {
            sidebar.focused = false;
            return None;
        }
        let mut action = None;
        match key.code {
            KeyCode::Esc => sidebar.focused = false,
            KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End => {
                let mut previous = None;
                let targets = sidebar
                    .document
                    .rows
                    .iter()
                    .enumerate()
                    .filter_map(|(index, row)| {
                        let unique = row.action.is_some() && row.action != previous;
                        previous = row.action;
                        unique.then_some(index)
                    })
                    .collect::<Vec<_>>();
                let current = targets
                    .iter()
                    .position(|index| Some(*index) == sidebar.focused_row)
                    .or_else(|| {
                        let action = sidebar
                            .focused_row
                            .and_then(|index| sidebar.document.rows.get(index))
                            .and_then(|row| row.action);
                        targets
                            .iter()
                            .position(|index| sidebar.document.rows[*index].action == action)
                    })
                    .unwrap_or(0);
                let next = match key.code {
                    KeyCode::Up => current.saturating_sub(1),
                    KeyCode::Down => (current + 1).min(targets.len().saturating_sub(1)),
                    KeyCode::End => targets.len().saturating_sub(1),
                    _ => 0,
                };
                sidebar.focused_row = targets.get(next).copied();
                if let Some(index) = sidebar.focused_row {
                    if let Some(region) =
                        regions(&sidebar.document, usize::from(layout.composer_top))
                            .iter()
                            .find(|region| {
                                region.header == index || region.content.contains(&index)
                            })
                    {
                        sidebar.scrolls.reveal(region, index);
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                action = sidebar
                    .focused_row
                    .and_then(|index| sidebar.document.rows.get(index))
                    .and_then(|row| row.action)
            }
            _ => return Some(Effect::Nothing),
        }
        if let Some(action) = action {
            self.apply_sidebar_action(action);
            return Some(Effect::Refresh);
        }
        Some(Effect::Repaint)
    }
}

/// 【终端侧栏】【行构造】集中生成文本与可选动作，禁止通过显示文本解析操作。
/// 参数: text 为 ANSI 文本，action 为可选操作；返回: 概览行。
pub(super) fn row(text: String, action: Option<SidebarAction>) -> SidebarRow {
    SidebarRow {
        text,
        action,
        selected: false,
    }
}
