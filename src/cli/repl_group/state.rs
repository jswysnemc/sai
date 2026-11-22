use crate::super_agent::model::{
    GroupDetailSnapshot, GroupMember, GroupSummary, MemberRole, WatchdogEvent,
};
use crate::super_agent::SuperAgentService;
use anyhow::Result;

/// 右侧检视面板标签页。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InspectorTab {
    TasksReports,
    ThreadsTriggers,
    ChannelsWatchdog,
}

impl InspectorTab {
    /// 循环切换下一个标签页。
    pub(super) fn next(self) -> Self {
        match self {
            Self::TasksReports => Self::ThreadsTriggers,
            Self::ThreadsTriggers => Self::ChannelsWatchdog,
            Self::ChannelsWatchdog => Self::TasksReports,
        }
    }

    /// 返回标签显示名。
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::TasksReports => "Tasks/Reports",
            Self::ThreadsTriggers => "Threads/Trig",
            Self::ChannelsWatchdog => "Chan/Watchdog",
        }
    }
}

/// 终端群组浮层弹窗类型。
#[derive(Debug, Clone)]
pub(super) enum GroupModal {
    CreateGroup {
        name: String,
        description: String,
        field: usize,
    },
    SpawnRunner {
        name: String,
        specialty: String,
        prompt: String,
        field: usize,
    },
    ChannelSim {
        channel: String,
        chat_id: String,
        actor: String,
        content: String,
        field: usize,
    },
}

impl GroupModal {
    /// 弹窗标题。
    pub(super) fn title(&self) -> &'static str {
        match self {
            Self::CreateGroup { .. } => "New Super-Agent Group",
            Self::SpawnRunner { .. } => "Spawn Worker (Runner)",
            Self::ChannelSim { .. } => "Channel -> Leader Message",
        }
    }

    /// 当前弹窗字段数。
    pub(super) fn field_count(&self) -> usize {
        match self {
            Self::CreateGroup { .. } => 2,
            Self::SpawnRunner { .. } => 3,
            Self::ChannelSim { .. } => 4,
        }
    }

    /// 切换下一个输入字段。
    pub(super) fn next_field(&mut self) {
        let count = self.field_count();
        match self {
            Self::CreateGroup { field, .. }
            | Self::SpawnRunner { field, .. }
            | Self::ChannelSim { field, .. } => *field = (*field + 1) % count,
        }
    }

    /// 读取当前激活字段的可变引用。
    pub(super) fn active_text_mut(&mut self) -> &mut String {
        match self {
            Self::CreateGroup {
                name,
                description,
                field,
            } => {
                if *field == 0 {
                    name
                } else {
                    description
                }
            }
            Self::SpawnRunner {
                name,
                specialty,
                prompt,
                field,
            } => match *field {
                0 => name,
                1 => specialty,
                _ => prompt,
            },
            Self::ChannelSim {
                channel,
                chat_id,
                actor,
                content,
                field,
            } => match *field {
                0 => channel,
                1 => chat_id,
                2 => actor,
                _ => content,
            },
        }
    }
}

/// 单帧绘制记录的可点击区域，供鼠标交互命中判定。
#[derive(Debug, Clone, Default)]
pub(super) struct GroupHitMap {
    /// `(screen_row, group_index)`
    pub(super) group_rows: Vec<(u16, usize)>,
    /// `(screen_row, member_index)`
    pub(super) member_rows: Vec<(u16, usize)>,
    /// 左侧边栏右边界列
    pub(super) left_end_col: u16,
    /// 群聊区域列范围 `[start, end)`
    pub(super) chat_col_range: (u16, u16),
    /// 群聊区域行范围 `[start, end)`
    pub(super) chat_row_range: (u16, u16),
    /// 右侧检视栏标签页点击区 `(row, start_col, end_col, tab)`
    pub(super) tab_hits: Vec<(u16, u16, u16, InspectorTab)>,
}

/// 群组 TUI 全量运行期视图状态。
pub(super) struct GroupTuiState {
    pub(super) groups: Vec<GroupSummary>,
    pub(super) selected_group_idx: usize,
    pub(super) detail: Option<GroupDetailSnapshot>,
    pub(super) watchdog_events: Vec<WatchdogEvent>,
    pub(super) selected_member_idx: usize,
    pub(super) inspector_open: bool,
    pub(super) inspector_tab: InspectorTab,
    pub(super) chat_scroll: usize,
    pub(super) input: String,
    pub(super) cursor: usize,
    pub(super) modal: Option<GroupModal>,
    pub(super) notice: Option<String>,
    pub(super) hit_map: GroupHitMap,
}

impl GroupTuiState {
    /// 初始化空白群组工作台状态。
    pub(super) fn new() -> Self {
        Self {
            groups: Vec::new(),
            selected_group_idx: 0,
            detail: None,
            watchdog_events: Vec::new(),
            selected_member_idx: 0,
            inspector_open: true,
            inspector_tab: InspectorTab::TasksReports,
            chat_scroll: 0,
            input: String::new(),
            cursor: 0,
            modal: None,
            notice: None,
            hit_map: GroupHitMap::default(),
        }
    }

    /// 从 Super-Agent 服务重新拉取群组列表与当前群组完整详情。
    ///
    /// 参数:
    /// - `service`: Super-Agent 核心服务
    /// - `select_target`: 可选目标群组 ID 或名称
    ///
    /// 返回:
    /// - 刷新是否成功
    pub(super) fn reload(
        &mut self,
        service: &SuperAgentService,
        select_target: Option<&str>,
    ) -> Result<()> {
        let current_id = self.selected_group_id().map(ToOwned::to_owned);
        let overview = service.overview()?;
        self.groups = overview.groups;
        self.watchdog_events = overview.recent_watchdog;

        if self.groups.is_empty() {
            self.selected_group_idx = 0;
            self.detail = None;
            return Ok(());
        }

        if let Some(target) = select_target {
            if let Some(idx) = self.groups.iter().position(|item| {
                item.group.id == target || item.group.name.eq_ignore_ascii_case(target)
            }) {
                self.selected_group_idx = idx;
            }
        } else if let Some(prev_id) = current_id {
            if let Some(idx) = self
                .groups
                .iter()
                .position(|item| item.group.id == prev_id)
            {
                self.selected_group_idx = idx;
            }
        }

        if self.selected_group_idx >= self.groups.len() {
            self.selected_group_idx = self.groups.len() - 1;
        }
        let active_id = self.groups[self.selected_group_idx].group.id.clone();
        let detail = service.group_detail(&active_id, 120)?;
        if self.selected_member_idx >= detail.members.len() {
            self.selected_member_idx = 0;
        }
        self.detail = Some(detail);
        Ok(())
    }

    /// 返回当前选中的群组 ID。
    pub(super) fn selected_group_id(&self) -> Option<&str> {
        self.groups
            .get(self.selected_group_idx)
            .map(|item| item.group.id.as_str())
    }

    /// 返回当前选中的成员。
    pub(super) fn selected_member(&self) -> Option<&GroupMember> {
        self.detail
            .as_ref()
            .and_then(|detail| detail.members.get(self.selected_member_idx))
    }

    /// 返回当前选中的可停止 Runner（若当前选中的是 Worker）。
    pub(super) fn selected_worker(&self) -> Option<&GroupMember> {
        self.selected_member()
            .filter(|member| member.role == MemberRole::Worker)
    }

    /// 设置底部状态栏提示消息。
    pub(super) fn set_notice(&mut self, text: impl Into<String>) {
        self.notice = Some(text.into());
    }

    /// 在输入框当前光标处插入字符。
    pub(super) fn insert_char(&mut self, ch: char) {
        let idx = self.cursor.min(self.input.len());
        self.input.insert(idx, ch);
        self.cursor = idx + ch.len_utf8();
    }

    /// 删除光标前一个字符。
    pub(super) fn backspace(&mut self) {
        if self.cursor == 0 || self.input.is_empty() {
            return;
        }
        let prev = self.input[..self.cursor]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        self.input.drain(prev..self.cursor);
        self.cursor = prev;
    }

    /// 向左移动输入光标。
    pub(super) fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor = self.input[..self.cursor]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0);
    }

    /// 向右移动输入光标。
    pub(super) fn move_right(&mut self) {
        if self.cursor >= self.input.len() {
            return;
        }
        if let Some(ch) = self.input[self.cursor..].chars().next() {
            self.cursor += ch.len_utf8();
        }
    }

    /// 在输入框前缀插入 `@成员名 `，便于快速定向与群组成员对话。
    pub(super) fn insert_member_mention(&mut self, member_name: &str) {
        let prefix = format!("@{member_name} ");
        if !self.input.starts_with(&prefix) {
            self.input = format!("{prefix}{}", self.input.trim_start());
            self.cursor = self.input.len();
        }
    }

    /// 切换上一个/下一个群组。
    pub(super) fn step_group(&mut self, delta: isize) -> bool {
        if self.groups.len() <= 1 {
            return false;
        }
        let len = self.groups.len() as isize;
        let next = (self.selected_group_idx as isize + delta).rem_euclid(len) as usize;
        if next != self.selected_group_idx {
            self.selected_group_idx = next;
            self.chat_scroll = 0;
            self.selected_member_idx = 0;
            true
        } else {
            false
        }
    }
}
