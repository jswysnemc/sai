//! 群组工作台运行期核心状态：统一字符级光标、输入历史、模型快照、侧栏标签页与鼠标热区。

pub(super) use super::completion::CompletionItem;
pub(super) use super::modal_state::GroupModal;
use super::model_info::GroupModelSnapshot;
use crate::cli::repl_text::{
    insert_char_at_cursor, insert_newline_at_cursor, insert_str_at_cursor,
    remove_char_at_cursor, remove_char_before_cursor,
};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::super_agent::model::{
    GroupDetailSnapshot, GroupMember, GroupSummary, MemberRole, WatchdogEvent,
};
use crate::super_agent::SuperAgentService;
use anyhow::Result;
use std::time::{Duration, Instant};

/// 普通操作提示在底栏停留的时长，超时后自动恢复工作区路径与分支显示。
const NOTICE_INFO_TTL: Duration = Duration::from_secs(4);
/// 错误提示在底栏停留的时长。
const NOTICE_ERROR_TTL: Duration = Duration::from_secs(8);

/// 右侧统一侧栏标签页（总览集成群组/成员/任务/报告，其余覆盖渠道、工具边界与守护审计）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InspectorTab {
    Overview,
    ChannelsGateways,
    ToolsDesign,
    ThreadsWatchdog,
}

impl InspectorTab {
    /// 循环切换下一个侧栏标签页。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 下一个标签页
    pub(super) fn next(self) -> Self {
        match self {
            Self::Overview => Self::ChannelsGateways,
            Self::ChannelsGateways => Self::ToolsDesign,
            Self::ToolsDesign => Self::ThreadsWatchdog,
            Self::ThreadsWatchdog => Self::Overview,
        }
    }

    /// 返回全部标签页有序列表。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 四个标签页数组
    pub(super) fn all() -> [Self; 4] {
        [
            Self::Overview,
            Self::ChannelsGateways,
            Self::ToolsDesign,
            Self::ThreadsWatchdog,
        ]
    }

    /// 返回紧凑标签显示名。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 标签中文名称
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Overview => "总览",
            Self::ChannelsGateways => "渠道",
            Self::ToolsDesign => "工具",
            Self::ThreadsWatchdog => "守护",
        }
    }
}

/// 渠道网关本地运行与登录状态摘要（供顶栏与右侧渠道检视页实时展示）。
#[derive(Debug, Clone, Default)]
pub(super) struct GatewayStatusSummary {
    pub(super) weixin_logged_in: bool,
    pub(super) weixin_running: bool,
    pub(super) qq_configured: bool,
    pub(super) qq_running: bool,
    pub(super) feishu_configured: bool,
    pub(super) channel_leader_routing: bool,
}

impl GatewayStatusSummary {
    /// 从本地配置、微信登录账号与托管进程记录同步读取实时网关状态。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 当前网关状态快照
    pub(super) fn probe(paths: &SaiPaths) -> Self {
        let config = AppConfig::load_or_default(paths).unwrap_or_default();
        let records = crate::gateways::process_control::refresh_gateway_processes(paths)
            .unwrap_or_default();
        let is_running = |id: &str| {
            records
                .iter()
                .any(|record| record.gateway_id == id && record.is_running())
        };
        let weixin_logged_in = !config.gateways.weixin.token.trim().is_empty()
            || crate::gateways::weixin_bot::login::load_weixin_account(
                paths,
                (!config.gateways.weixin.account.trim().is_empty())
                    .then_some(config.gateways.weixin.account.trim()),
            )
            .is_ok();
        let qq_configured = !config.gateways.qq.app_id.trim().is_empty()
            || !config.gateways.qq.token.trim().is_empty();
        let feishu_configured = !config.gateways.feishu.app_id.trim().is_empty()
            && !config.gateways.feishu.app_secret.trim().is_empty();
        Self {
            weixin_logged_in,
            weixin_running: is_running("weixin"),
            qq_configured,
            qq_running: is_running("qq"),
            feishu_configured,
            channel_leader_routing: config.super_agent.channel_leader_routing,
        }
    }
}

/// Leader 实时流式执行状态（展示正在调用的工具与流式草稿，避免等待黑盒）。
#[derive(Debug, Clone, Default)]
pub(super) struct LeaderLiveState {
    pub(super) thinking: bool,
    pub(super) activity: String,
    pub(super) draft_preview: String,
}

/// 单帧绘制记录的可点击区域，供鼠标悬停、点击与滚轮精准定位。
#[derive(Debug, Clone, Default)]
pub(super) struct GroupHitMap {
    /// 顶栏右上角侧栏开关列区间 `[start, end)`（位于第 0 行）
    pub(super) header_sidebar_toggle: Option<(u16, u16)>,
    /// 聊天下方“回到底部”悬浮胶囊 `(row, start_col, end_col)`
    pub(super) bottom_button: Option<(u16, u16, u16)>,
    /// 底栏模型/思考信息可点击区 `(row, start_col, end_col)`
    pub(super) footer_model_hit: Option<(u16, u16, u16)>,
    /// 右侧栏 Leader 模型摘要可点击行号
    pub(super) sidebar_model_row: Option<u16>,
    /// 补全面板候选行 `(row, completion_index)`
    pub(super) completion_hits: Vec<(u16, usize)>,
    /// 补全面板列宽上限（避免右侧栏误命中）
    pub(super) completion_end_col: u16,
    /// 侧栏群组条目 `(screen_row, group_index)`
    pub(super) group_rows: Vec<(u16, usize)>,
    /// 侧栏成员条目 `(screen_row, member_index)`
    pub(super) member_rows: Vec<(u16, usize)>,
    /// 右侧栏标签页点击区 `(row, start_col, end_col, tab)`
    pub(super) tab_hits: Vec<(u16, u16, u16, InspectorTab)>,
    /// 右侧栏起始列（用于滚轮分区与点击判定）
    pub(super) sidebar_start_col: u16,
    /// 右侧栏正文起止行 `[start, end)`
    pub(super) sidebar_body_rows: (u16, u16),
    /// 主聊区域列范围 `[start, end)`
    pub(super) chat_col_range: (u16, u16),
    /// 主聊区域行范围 `[start, end)`
    pub(super) chat_row_range: (u16, u16),
    /// 原生输入区行范围 `[start, end)`
    pub(super) composer_row_range: (u16, u16),
    /// 输入框内容可用列数（用于多行视觉光标上下移动）
    pub(super) composer_content_cols: usize,
}

/// 群组 TUI 全量运行期视图状态。
pub(super) struct GroupTuiState {
    pub(super) groups: Vec<GroupSummary>,
    pub(super) selected_group_idx: usize,
    pub(super) detail: Option<GroupDetailSnapshot>,
    pub(super) watchdog_events: Vec<WatchdogEvent>,
    pub(super) gateway_status: GatewayStatusSummary,
    pub(super) model_info: GroupModelSnapshot,
    pub(super) leader_live: LeaderLiveState,
    pub(super) selected_member_idx: usize,
    pub(super) inspector_open: bool,
    pub(super) inspector_tab: InspectorTab,
    pub(super) inspector_scroll: usize,
    pub(super) chat_scroll: usize,
    pub(super) chat_total_lines: usize,
    pub(super) chat_viewport_rows: usize,
    pub(super) unseen_chat: bool,
    /// 输入文本草稿
    pub(super) input: String,
    /// 输入框光标字符下标（与 `repl_text` / `repl_input::shortcuts` 保持一致）
    pub(super) cursor: usize,
    /// 本次群组会话内提交过的输入历史
    pub(super) input_history: Vec<String>,
    /// 当前浏览到的历史索引（等于 `input_history.len()` 时表示当前草稿）
    pub(super) history_idx: usize,
    /// 进入历史浏览前暂存的当前草稿
    pub(super) history_draft: String,
    pub(super) completion_idx: usize,
    /// 用户按 Esc 主动收起当前补全面板，修改输入后自动复位
    pub(super) completions_dismissed: bool,
    pub(super) modal: Option<GroupModal>,
    pub(super) notice: Option<String>,
    pub(super) notice_is_error: bool,
    pub(super) notice_set_at: Option<Instant>,
    pub(super) tick: usize,
    /// 当前鼠标指针终端坐标 `(col, row)`
    pub(super) pointer: Option<(u16, u16)>,
    pub(super) hit_map: GroupHitMap,
}

impl GroupTuiState {
    /// 初始化群组工作台默认状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认群组 TUI 状态
    pub(super) fn new() -> Self {
        Self {
            groups: Vec::new(),
            selected_group_idx: 0,
            detail: None,
            watchdog_events: Vec::new(),
            gateway_status: GatewayStatusSummary::default(),
            model_info: GroupModelSnapshot::default(),
            leader_live: LeaderLiveState::default(),
            selected_member_idx: 0,
            inspector_open: true,
            inspector_tab: InspectorTab::Overview,
            inspector_scroll: 0,
            chat_scroll: 0,
            chat_total_lines: 0,
            chat_viewport_rows: 1,
            unseen_chat: false,
            input: String::new(),
            cursor: 0,
            input_history: Vec::new(),
            history_idx: 0,
            history_draft: String::new(),
            completion_idx: 0,
            completions_dismissed: false,
            modal: None,
            notice: None,
            notice_is_error: false,
            notice_set_at: None,
            tick: 0,
            pointer: None,
            hit_map: GroupHitMap::default(),
        }
    }

    /// 【群组终端】【数据刷新】从 Super-Agent 服务重新获取群组列表、详情、网关状态与模型快照。
    ///
    /// 参数:
    /// - `service`: Super-Agent 核心服务
    /// - `select_target`: 可选指定切换的目标群组 ID 或名称
    ///
    /// 返回:
    /// - 刷新结果
    pub(super) fn reload(
        &mut self,
        service: &SuperAgentService,
        select_target: Option<&str>,
    ) -> Result<()> {
        let current_id = self.selected_group_id().map(ToOwned::to_owned);
        let prev_msg_count = self.detail.as_ref().map(|d| d.messages.len()).unwrap_or(0);
        let overview = service.overview()?;
        self.groups = overview.groups;
        self.watchdog_events = overview.recent_watchdog;
        self.gateway_status = GatewayStatusSummary::probe(service.paths());

        if self.groups.is_empty() {
            self.selected_group_idx = 0;
            self.detail = None;
            self.model_info = GroupModelSnapshot::probe(service.paths(), None, None);
            return Ok(());
        }

        if let Some(target) = select_target {
            if let Some(idx) = self.groups.iter().position(|item| {
                item.group.id == target || item.group.name.eq_ignore_ascii_case(target)
            }) {
                self.selected_group_idx = idx;
            }
        } else if let Some(prev_id) = current_id.as_deref() {
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
        let switched = current_id.as_deref() != Some(active_id.as_str());
        let detail = service.group_detail(&active_id, 120)?;
        if self.selected_member_idx >= detail.members.len() {
            self.selected_member_idx = 0;
        }
        if !switched && self.chat_scroll > 0 && detail.messages.len() > prev_msg_count {
            self.unseen_chat = true;
        }
        if self.chat_scroll == 0 {
            self.unseen_chat = false;
        }

        // 1. 同步 Leader 数据库运行态，避免跨进程运行网关时丢失思考状态
        let leader_running = detail
            .members
            .iter()
            .find(|m| m.role == MemberRole::Leader)
            .is_some_and(|m| m.status == crate::super_agent::model::MemberStatus::Running);
        if leader_running && !self.leader_live.thinking {
            self.leader_live.thinking = true;
            if let Some(leader) = detail.members.iter().find(|m| m.role == MemberRole::Leader) {
                self.leader_live.activity = leader.last_activity.clone();
            }
        } else if !leader_running {
            self.leader_live = LeaderLiveState::default();
        }

        // 2. 同步当前群组生效的模型基础信息与上下文占用快照
        self.model_info = GroupModelSnapshot::probe(
            service.paths(),
            Some(&detail.group.id),
            Some(&detail.group.workspace_path),
        );
        self.detail = Some(detail);
        Ok(())
    }

    /// 返回当前选中的群组 ID。
    pub(super) fn selected_group_id(&self) -> Option<&str> {
        self.groups
            .get(self.selected_group_idx)
            .map(|item| item.group.id.as_str())
    }

    /// 返回当前选中的群组成员。
    pub(super) fn selected_member(&self) -> Option<&GroupMember> {
        self.detail
            .as_ref()
            .and_then(|detail| detail.members.get(self.selected_member_idx))
    }

    /// 返回当前选中的 Worker 成员（仅当选中项为 Runner 时）。
    pub(super) fn selected_worker(&self) -> Option<&GroupMember> {
        self.selected_member()
            .filter(|member| member.role == MemberRole::Worker)
    }

    /// 设置底部普通提示信息。
    pub(super) fn set_notice(&mut self, text: impl Into<String>) {
        self.notice = Some(text.into());
        self.notice_is_error = false;
        self.notice_set_at = Some(Instant::now());
    }

    /// 设置底部错误提示信息。
    pub(super) fn set_error(&mut self, text: impl Into<String>) {
        self.notice = Some(text.into());
        self.notice_is_error = true;
        self.notice_set_at = Some(Instant::now());
    }

    /// 清理非错误的瞬态提示（切换标签、打开弹窗或切换群组时立即恢复工作区路径显示）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn clear_transient_notice(&mut self) {
        if !self.notice_is_error {
            self.notice = None;
            self.notice_set_at = None;
        }
    }

    /// 清理已超过展示时限的底栏提示，恢复工作区目录与 Git 分支显示。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn expire_notice(&mut self) {
        let Some(since) = self.notice_set_at else {
            return;
        };
        let ttl = if self.notice_is_error {
            NOTICE_ERROR_TTL
        } else {
            NOTICE_INFO_TTL
        };
        if since.elapsed() >= ttl {
            self.notice = None;
            self.notice_is_error = false;
            self.notice_set_at = None;
        }
    }

    /// 在输入框当前字符光标处插入单个字符。
    pub(super) fn insert_char(&mut self, ch: char) {
        self.clamp_cursor();
        insert_char_at_cursor(&mut self.input, &mut self.cursor, ch);
        self.on_input_mutated();
    }

    /// 在输入框当前字符光标处插入多字符文本（保留换行）。
    pub(super) fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.clamp_cursor();
        insert_str_at_cursor(&mut self.input, &mut self.cursor, text);
        self.on_input_mutated();
    }

    /// 在输入框当前光标处插入换行符。
    pub(super) fn insert_newline(&mut self) {
        self.clamp_cursor();
        insert_newline_at_cursor(&mut self.input, &mut self.cursor);
        self.on_input_mutated();
    }

    /// 删除光标前一个字符。
    pub(super) fn backspace(&mut self) {
        self.clamp_cursor();
        if self.cursor == 0 || self.input.is_empty() {
            return;
        }
        remove_char_before_cursor(&mut self.input, &mut self.cursor);
        self.on_input_mutated();
    }

    /// 删除光标所在位置的后向字符（Delete 键）。
    pub(super) fn delete_forward(&mut self) {
        self.clamp_cursor();
        if self.cursor >= self.input.chars().count() {
            return;
        }
        remove_char_at_cursor(&mut self.input, self.cursor);
        self.on_input_mutated();
    }

    /// 光标左移一个字符。
    pub(super) fn move_left(&mut self) {
        self.clamp_cursor();
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// 光标右移一个字符。
    pub(super) fn move_right(&mut self) {
        let len = self.input.chars().count();
        self.cursor = (self.cursor + 1).min(len);
    }

    /// 将当前输入记录进历史并重置浏览指针。
    pub(super) fn push_history(&mut self, entry: &str) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.input_history.last().map(String::as_str) != Some(trimmed) {
            self.input_history.push(trimmed.to_string());
            if self.input_history.len() > 100 {
                self.input_history.remove(0);
            }
        }
        self.history_idx = self.input_history.len();
        self.history_draft.clear();
    }

    /// 向上翻阅上一条历史输入。
    pub(super) fn history_up(&mut self) -> bool {
        if self.input_history.is_empty() || self.history_idx == 0 {
            return false;
        }
        if self.history_idx == self.input_history.len() {
            self.history_draft = self.input.clone();
        }
        self.history_idx -= 1;
        self.input = self.input_history[self.history_idx].clone();
        self.cursor = self.input.chars().count();
        self.completion_idx = 0;
        self.completions_dismissed = false;
        true
    }

    /// 向下翻阅下一条历史输入或回到当前草稿。
    pub(super) fn history_down(&mut self) -> bool {
        if self.history_idx >= self.input_history.len() {
            return false;
        }
        self.history_idx += 1;
        self.input = if self.history_idx == self.input_history.len() {
            std::mem::take(&mut self.history_draft)
        } else {
            self.input_history[self.history_idx].clone()
        };
        self.cursor = self.input.chars().count();
        self.completion_idx = 0;
        self.completions_dismissed = false;
        true
    }

    /// 在输入框前缀插入 `@成员名 `，便于快速与指定成员对话。
    pub(super) fn insert_member_mention(&mut self, member_name: &str) {
        let prefix = format!("@{member_name} ");
        if !self.input.starts_with(&prefix) {
            self.input = format!("{prefix}{}", self.input.trim_start());
            self.cursor = self.input.chars().count();
            self.on_input_mutated();
        }
    }

    /// 切换上一个或下一个群组。
    pub(super) fn step_group(&mut self, delta: isize) -> bool {
        if self.groups.len() <= 1 {
            return false;
        }
        let len = self.groups.len() as isize;
        let next = (self.selected_group_idx as isize + delta).rem_euclid(len) as usize;
        if next != self.selected_group_idx {
            self.selected_group_idx = next;
            self.chat_scroll = 0;
            self.unseen_chat = false;
            self.inspector_scroll = 0;
            self.selected_member_idx = 0;
            self.leader_live = LeaderLiveState::default();
            true
        } else {
            false
        }
    }

    /// 在成员列表中上下移动选中项。
    pub(super) fn step_member(&mut self, delta: isize) {
        let count = self.detail.as_ref().map(|d| d.members.len()).unwrap_or(0);
        if count <= 1 {
            return;
        }
        self.selected_member_idx =
            (self.selected_member_idx as isize + delta).rem_euclid(count as isize) as usize;
    }

    /// 确保字符光标不超出当前文本长度。
    pub(super) fn clamp_cursor(&mut self) {
        let len = self.input.chars().count();
        if self.cursor > len {
            self.cursor = len;
        }
    }

    /// 输入文本变更时复位补全收起态、历史浏览指针与过期的普通提示。
    pub(super) fn on_input_mutated(&mut self) {
        self.completion_idx = 0;
        self.completions_dismissed = false;
        self.history_idx = self.input_history.len();
        if !self.notice_is_error {
            self.notice = None;
            self.notice_set_at = None;
        }
    }
}
